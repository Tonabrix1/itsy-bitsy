use reqwest::Client;
use reqwest::header::USER_AGENT;
use tokio;
use select::document::Document;
use select::predicate::Name;
use std::collections::{HashMap, HashSet};
use std::env;
use std::iter::Iterator;
use std::time::{Instant, Duration};
use url::Url;

// Structure meant to hold optional recursive references in a HashMap
/*#[derive(Clone)]
struct Branch {
    pub key: String,
    pub body: HashMap<String, Option<& Branch>>,
}*/

/**
* scope : the base domain - Example: https://www.example.com
* domain : the full url including the domain and whatever directory/file you would like to begin searching from - Example: https://www.example.com/search_me
* done : indicates whether the spider has completed the crawling operation
* tree : the root Branch of the directory map
*/
#[derive(Clone)]
struct Spider {
    pub scope: String,
    pub domain: String,
    pub done: i16,
    pub found: i16,
    pub harvested: HashSet<String>,
    pub queued: HashSet<String>,
    pub client: reqwest::Client,
}

/**
* domain : the base domain to search, the base domain will be used as the scope for now
*
*
*/
fn spawn(domain: String) -> Spider {
    println!("Generating spider for host: {:?}", domain);
    Spider {
        scope: get_scope(domain.clone()),
        domain: domain.clone(),
        done: 0,
        found: 0,
        harvested: HashSet::new(),
        queued: HashSet::new(),
        client: Client::new(),
    }
}

/** (async reqwest::get implemented for future extension, but awaits are used to make it functionally synchronous)
* spider : the Spider object to begin the search with
* args : command line argument vector
* mutates the spider by updating new links and crawling those directories
*
* RET : a spider that has been updated with it's findings after searching a single directory
*/
async fn crawl(mut spider: Spider, args: Vec<String>) -> Spider{
    println!("crawling {:?}", spider.domain);
    let resp = spider.client.get(spider.domain.clone()).header(USER_AGENT, "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:96.0) Gecko/20100101 Firefox/96.0").send().await.unwrap().text().await.unwrap();
    let mut parsed = parse_a(resp.clone()).await;
    spider.done += 1;
    let harvested = harvest(spider.domain.clone());

    spider.harvested.insert(harvested.clone());

    //for each link found in the anchor tags
    for p in parsed {
        let p = format_link(p, spider.scope.clone()).await;
        if p.len() > 0 && !is_found(p.clone(), spider.clone()) {
            spider.queued.insert(p);
            spider.found += 1;
        }
    }

    if !args.contains(&"--noimage".to_string()) && !args.contains(&"-n".to_string()) {
        parsed = parse_img(resp).await;
        for p in parsed {
            let p = format_link(p, spider.scope.clone()).await;
            if p.len() > 0 && !is_found(p.clone(), spider.clone()) {
                spider.queued.insert(p);
                spider.found += 1;
            }
        }
    }

    println!("Harvested\n{:?}", spider.harvested.clone());
    println!("\n\nqueued #:\n{:?}", spider.found.clone());
    println!("\n\nharvested #:\n{:?}", spider.done.clone());
    println!("Queued\n{:?}", spider.queued.clone());
    // WIP
    //print_tree(&spider.tree.clone());

    if let Some(next) = spider.queued.iter().next() {
        spider.domain = next.to_string();
        &spider.queued.remove(&next.to_string());
    }
    spider
}

/**
* host : the string to format
* scope: the base domain that the spider is searching
* TODO: add a scope vector that allows multiple domains to be entered into scope (whitelisting)
* TODO: add a flag and parameter for a Vector<String> of scopes to avoid instead of using whitelisting(blacklisting)
*/
async fn format_link(mut host: String, mut scope: String) -> String {
    //truncate the ur
    for c in ['#', '?'] {
        if host.contains(c) {
            let splits: Vec<&str> = host.split(c).collect();
            host = splits[0].to_string();
        }
    }
    if host.starts_with(&scope) {
        return host;
    }
    if host.starts_with("/") {
        scope.push_str(&host);
        return scope;
    }
    String::new()
}

//create a new branch with an empty child vector
/*fn new_branch(path: String) -> & mut Branch {
    let mut ret = Branch {
        key: path.clone(),
        body: HashMap::new(),
    };
    ret.body.insert(path, None);
    &mut ret
}*/

//stores a branch in a tree of hashmaps in order to map the filesystem correctly
/*fn store_branch(path: String, root: & mut Branch) -> & mut Branch{
    // harvested link
    let splits: Vec<&str> = path.split('/').collect();
    let top: & mut Branch = root;
    for dir in splits {
        if dir.clone().len() <= 0 { continue; }
        //println!("cacheing dir: {:?}", dir.clone());
        match top.body.get(dir.clone()) {
            Some(x) => top = &mut x.clone().unwrap(),
            None => {
                top.body.remove(dir.clone());
                top.body.insert(dir.to_string(), Some(new_branch(dir.to_string())));
                top = &mut top.body.get(dir.clone()).unwrap().clone().unwrap();
            },
        }
    }
    root
}*/

//chops directory off of the base domain - Example: `https://www.example.com/this/is/a/test/` -> `/this/is/a/test/`
fn harvest(host: String) -> String {
    let url = Url::parse(&host).unwrap();
    url.path().to_string()
}

//check if the dir has been harvested already
fn is_found(host: String, spider: Spider) -> bool {
    let url = harvest(host);
    spider.harvested.contains(&url) || spider.queued.contains(&url)
}

//get the raw domain - Example: `https://www.example.com/test` -> `https://www.example.com`
fn get_scope(mut host: String) -> String {
    let url = Url::parse(&host).unwrap();
    let path = url.path();
    host = host
        .clone()
        .strip_suffix(&path)
        .unwrap_or(&host)
        .to_string();
    println!("{:?}", path);
    host
}

//find all links in anchor tags
async fn parse_a(body: String) -> Vec<String> {
    Document::from_read(body.as_bytes())
        .unwrap()
        .find(Name("a"))
        .filter_map(|n| n.attr("href"))
        .map(|x| x.to_string())
        .collect()
}

//find all links in image tags
async fn parse_img(body: String) -> Vec<String> {
    Document::from_read(body.as_bytes())
        .unwrap()
        .find(Name("img"))
        .filter_map(|n| n.attr("src"))
        .map(|x| x.to_string())
        .collect()
}

//recursively prints the tree
/*fn print_tree(root: & Branch) {
    for val in root.body.get(&root.key).iter() {
        println!("Branch: {:?}", root.key);
        match val {
            Some(v) => print_tree(v.clone()),
            None => println!("Empty branch"),
        }
    }
}*/

#[tokio::main]
async fn main() {
    //grab the first command line arg
    let args: Vec<String> = env::args().collect();

    //validate the input
    if args.len() <= 1 {
        panic!("Usage: Cargo run <host>");
    }

    //generate a spider using the domain
    let mut spider: Spider = spawn(args[1].clone());
    let mut i : i16 = 0;
    let now = Instant::now();
    let z = Duration::from_secs(0);
    //crawl until all in scope links are exhausted
    while spider.clone().done == 0 || spider.clone().done < spider.clone().found {
        let total : i16 = spider.clone().found + spider.clone().done;
        let mut curr = now.elapsed();
        if curr > z {
            println!("Link #{:?}\nElapsed duration: {:?}s\nTotal links scraped: {:?}\nLinks per second: {:?}", i, curr, total, total as f32/curr.as_secs_f32());
        }
        i += 1;
        spider = crawl(spider, args.clone()).await;
    }
}
