extern crate log;
extern crate syslog;

use anyhow::{Context, Result};
use aws_config::imds;
use log::LevelFilter;
use regex::Regex;
use std::env;
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use syslog::{BasicLogger, Facility, Formatter3164};
use tokio::time::{timeout, Duration};
use users::get_user_by_uid;

const TIMEOUT: Duration = Duration::from_secs(5);
const VALID_DOMAINS: [&str; 4] = [
    "amazonaws.com",
    "amazonaws.com.cn",
    "c2s.ic.gov",
    "sc2s.sgov.gov",
];

async fn eic_curl_authorized_keys() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let user = &args[1];
    let umask = Permissions::from_mode(0o077);

    let imds = imds::Client::builder().build();

    let instance_id = imds.get("/latest/meta-data/instance-id/").await?;
    let instance_id_regex = Regex::new("^i-[0-9a-f]{8,32}$")?;
    // Validate the instance ID is i-abcd1234 (8 or 17 char, hex)
    // We have it buffered to 32 chars to futureproof any further EC2 format changes (given some other EC2 resources are already 32 char)
    (instance_id_regex.is_match(instance_id.as_ref()) &&
     // Verify we have an EC2 uuid
     (!Path::new("/sys/hypervisor/uuid").exists() ||
      // Nitro, switch to DMI check
      (fs::read_to_string(Path::new("/sys/devices/virtual/dmi/id/board_asset_tag"))? == instance_id.as_ref()
      )) ||
     // Leading bytes are not "ec2"
     fs::read_to_string(Path::new("/sys/hypervisor/uuid"))?.starts_with("ec2")
    ).then_some("instance_id").context("Could not verify running on ec2")?;
    let user = get_user_by_uid(user.parse()?).context(
        "EC2 Instance Connect was invoked without a user to authorize and will do nothing.",
    )?;
    let keys = imds
        .get(format!(
            "/managed-ssh-keys/active-keys/{}/",
            user.name().to_str().context("could not convert string")?
        ))
        .await?;
    let zone = imds.get("/placement/availability-zone/").await?;

    // Validate the zone is aa-bb-#c (or aa-bb-cc-#d for special partitions like AWS GovCloud)
    let az_regex = Regex::new("^([a-z]+-){2,3}[0-9][a-z]$")?;
    az_regex
        .is_match(zone.as_ref())
        .then_some("az")
        .context("Invalid availability zone")?;
    let domain = imds.get("/services/domain/").await?;
    VALID_DOMAINS
        .as_slice()
        .contains(&domain.as_ref())
        .then_some("domain")
        .context("EC2 Instance Connect found an invalid domain and will do nothing.")?;
    let region_regex = Regex::new("(([a-z]+-)+[0-9]+).*")?;
    let region = region_regex
        .captures(zone.as_ref())
        .context("Invalid AZ")?
        .get(1)
        .context("Invalid AWS region")?
        .as_str();
    let expected_signer = format!("managed-ssh-signer.{}.{}", region, domain.as_ref());

    Ok(())
}

#[tokio::main]
async fn main() {
    let formatter = Formatter3164 {
        facility: Facility::LOG_AUTHPRIV,
        hostname: None,
        process: "ec2-instance-connect".into(),
        pid: 0,
    };

    let logger = syslog::unix(formatter).expect("could not connect to syslog");
    log::set_boxed_logger(Box::new(BasicLogger::new(logger)))
        .map(|_| log::set_max_level(LevelFilter::Info))
        .expect("could not set up logger");

    let res = timeout(TIMEOUT, eic_curl_authorized_keys()).await;

    if res.is_err() {
        println!("operation timed out");
    }
}
