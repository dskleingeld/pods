use eyre::{eyre, Result, WrapErr};
use regex::Regex;
use super::{ApiBudget, SearchResult, APP_USER_AGENT};

#[derive(Clone)]
pub struct Search {
    client: reqwest::Client,
    title: Regex,
    url: Regex,
    budget: ApiBudget,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent(APP_USER_AGENT)
                .build()
                .wrap_err("could not construct http client for podcast searching").unwrap(),
            title: Regex::new(r#"collectionName":"(.+?)""#).unwrap(),
            url: Regex::new(r#"feedUrl":"(.+?)""#).unwrap(),
            budget: ApiBudget::from(5),
        }
    }
}

impl Search {
    pub fn to_results(&self, text: &str) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        for (cap1, cap2) in self.title.captures_iter(text)
            .zip(self.url.captures_iter(text)){

            results.push(SearchResult {
                title: cap1.get(1)
                    .ok_or_else(|| eyre!("malformed title result"))?
                    .as_str().to_owned(),
                url: cap2.get(1)
                    .ok_or_else(|| eyre!("malformed url result"))?
                    .as_str().to_owned(),
            });
        }
        Ok(results) 
    }

    pub async fn search(&mut self, search_term: &str, ignore_budget: bool)
        -> Result<Vec<SearchResult>> {
        
        if self.budget.left() <= 2 && !ignore_budget {
            return Err(eyre!("over api budget"));
        }

        self.budget.register_call();
        let text = self.client.get("https://itunes.apple.com/search")
            .timeout(std::time::Duration::from_millis(1000))
            .query(&[("entity","podcast")])
            .query(&[("term",search_term)])
            .query(&[("limit",25)])
            .query(&[("explicit","Yes")])
            .send()
            .await
            .wrap_err("could not connect to apple podcasts")?
            .error_for_status()
            .wrap_err("server replied with error")?
            .text()
            .await
            .wrap_err("could not understand apple podcast reply")?;

        let results = self.to_results(&text)?;
        Ok(results)
    }
}

#[test]
fn test_apple_podcasts(){
    use tokio::runtime::Runtime;

    let mut searcher = Search::default();
    // Create the runtime
    Runtime::new()
        .unwrap()
        .block_on(async {
            let res = searcher.search("Soft Skills", true).await.unwrap();
            assert_eq!(res[0].title, "Soft Skills Engineering");
            assert_eq!(res[0].url, "http://feeds.feedburner.com/SoftSkillsEngineering");
        });
}
