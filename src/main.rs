use inquire::Confirm;
use inquire::Select;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u8,
    phones: Vec<String>,
}

static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);

static BOTOCORE_ROOT: &str = "https://api.github.com/repos/boto/botocore/contents/botocore/data";

#[derive(Serialize, Deserialize, Debug)]
struct Locations {
    #[serde(rename = "self")]
    api: String,
    git: String,
    html: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubRef {
    name: String,
    path: String,
    download_url: Option<String>,
    #[serde(rename = "_links")]
    locations: Locations,
}


fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::builder()
        .user_agent(USER_AGENT.to_owned())
        .build()?;

    // Load all of the available services
    let services_response = client.get(BOTOCORE_ROOT)
        .header("Accept", "application/vnd.github.v3+json")
        .send()?
        .error_for_status()
        .expect("Hit an error querying GitHub");

    let services : Vec<GitHubRef> = services_response.json()?;
    let services_options = services.iter().map(|s| s.name.as_str()).collect::<Vec<&str>>();
    let services_ans = Select::new("Select AWS SDK:", services_options).raw_prompt()?;
    let service = &services[services_ans.index];

    // Load all available versions of the service
    // TODO(dastbe): we should just skip prompting the user if there is only one version
    let versions_response = client.get(service.locations.api.as_str())
        .header("Accept", "application/vnd.github.v3+json")
        .send()?
        .error_for_status()
        .expect("Hit an error querying GitHub");

    let versions : Vec<GitHubRef> = versions_response.json()?;
    let versions_options = versions.iter().map(|s| s.name.as_str()).collect::<Vec<&str>>();
    let versions_ans = Select::new("Select API version (hint: newer is usually better):", versions_options).raw_prompt()?;
    let version = &versions[versions_ans.index];

    // before we do anything to crazy, prompt the user first
    let confirm_write = Confirm::new(format!("Do you want download the latest SDK for {}/{}? This will write files to your disk!", service.name, version.name).as_ref())
        .with_default(false)
        .prompt()?;

    if !confirm_write {
        std::process::exit(-1);
    }

    let files_response = client.get(version.locations.api.as_str())
        .header("Accept", "application/vnd.github.v3+json")
        .send()?
        .error_for_status()
        .expect("Hit an error querying GitHub");
    let files : Vec<GitHubRef> = files_response.json()?;

    // this is more than a little hacky. in theory, we could get the model file and use the aws cli
    // to write it. HOWEVER, the cli does not make it easy to also include ancillary files like the
    // paginators, which if not present will subtly break apis by not enumerating all results.
    let directory = format!("{}/.aws/models/{}/{}", env!("HOME"), service.name, version.name);
    fs::create_dir_all(&directory).expect("Failed to create model directory");

    for file in files.iter() {
        let filename = format!("{}/{}", &directory, file.name);
        let mut file_on_disk = std::fs::File::create(filename)?;

        client.get(file.download_url.as_ref().expect("Found a directory where we shouldn't have"))
            .send()?
            .error_for_status()
            .expect("Hit an error querying GitHub")
            .copy_to(&mut file_on_disk)?;
    }

    Ok(())
}
