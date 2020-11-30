use iced::Length;
use iced::{button, Button, Column, Command, Element, Text, HorizontalAlignment};
use iced::{scrollable, Scrollable};
use iced::{TextInput, text_input};
use tokio::sync::Mutex;
use eyre::WrapErr;
use std::sync::Arc;

use crate::feed;
use crate::database;

#[derive(Debug, Clone)]
pub enum Message {
    SearchSubmit,
    SearchInputChanged(String),
    SearchResults(Vec<feed::SearchResult>),
    AddPodcast(String), //url
}

impl Into<crate::Message> for Message {
    fn into(self) -> crate::Message {
        crate::Message::Podcasts(self)
    }
}

#[derive(Default)]
pub struct Search {
    input: text_input::State,
    input_value: String, 
    search: Arc<Mutex<feed::Search>>,
}

impl Search {
    pub fn update(&mut self, message: Message) -> Command<crate::Message> {
        match message {
            Message::SearchSubmit => {
                // always do a web search if a search was submitted
                let term = self.input_value.clone();
                let search = self.search.clone();
                Command::perform(
                    async move {search.lock().await.search(term, true).await.unwrap()},
                    |r| crate::Message::Podcasts(Message::SearchResults(r))
            )}
            Message::SearchInputChanged(s) => {
                self.input_value = s;
                if feed::valid_url(&self.input_value) {
                    todo!();
                    Command::none()
                } else if self.input_value.len() > 4 {
                    let term = self.input_value.clone();
                    let search = self.search.clone();
                    Command::perform(
                        async move {search.lock().await.search(term, true).await.unwrap()},
                        |r| crate::Message::Podcasts(Message::SearchResults(r)) )
                } else {
                    Command::none() 
                }
            }
            Message::SearchResults(_) => {
                Command::none()
            }
            Message::AddPodcast(_) => {
                panic!("should never handle addpodcast in Search::update")
            }
        }
    }
    pub fn view(&mut self) -> TextInput<crate::Message> {
        TextInput::new(
            &mut self.input, 
            "Add podcast url", 
            &self.input_value, 
            |s| crate::Message::Podcasts(Message::SearchInputChanged(s)),
        ) 
        .width(Length::Fill)
        .on_submit(crate::Message::Podcasts(Message::SearchSubmit))
    }
    pub fn reset(&mut self) {
        self.input_value.clear();
    }
}

#[derive(Default)]
struct List {
    podcast_buttons: Vec<button::State>,
    podcast_names: Vec<String>,
    feedres_buttons: Vec<button::State>,
    feedres_info: Vec<feed::SearchResult>,
    scroll_state: scrollable::State,
}

fn feedres_button(button: &mut button::State, res: feed::SearchResult) -> Button<crate::Message> {
    Button::new(button, 
        Text::new(res.title).horizontal_alignment(HorizontalAlignment::Center)
    )
    //Todo replace content of ToEpisode with some key
    .on_press(crate::Message::Podcasts(Message::AddPodcast(res.url)))
    .padding(12)
    .width(Length::Fill)
}
fn podcast_button(button: &mut button::State, text: String, id: u64) -> Button<crate::Message> {
    Button::new(button, 
        Text::new(text).horizontal_alignment(HorizontalAlignment::Center)
    )
    //Todo replace content of ToEpisode with some key
    .on_press(crate::Message::ToEpisodes(id))
    .padding(12)
    .width(Length::Fill)
}

impl List {
    fn view(&mut self, search_term: &str) -> Scrollable<crate::Message> {
        let mut scrollable = Scrollable::new(&mut self.scroll_state)
            .padding(10)
            .height(iced::Length::Fill);
        for (button, info) in self.feedres_buttons.iter_mut()
            .zip(self.feedres_info.iter()) {

            scrollable = scrollable.push(feedres_button(button, info.clone()));
        }
        for (button, name) in self.podcast_buttons.iter_mut()
            .zip(self.podcast_names.iter().filter(|n| n.contains(search_term))) {

            let id = 0;
            scrollable = scrollable.push(podcast_button(button, name.to_owned(), id));
        }
        scrollable
    }
    fn update_feedres(&mut self, results: Vec<feed::SearchResult>) {
        //TODO add feedres_buttons
        self.feedres_info = results;
        let needed_buttons = self.feedres_info.len().saturating_sub(self.feedres_buttons.len());
        dbg!(&needed_buttons);
        for _ in 0..needed_buttons {
            self.feedres_buttons.push(button::State::new());
        }
    }
}

pub struct Podcasts {
    /// the podcasts title
    list: List,
    search: Search,
    podcasts: database::Podcasts,
    // possible opt to do, cache the view
}

impl Podcasts {
    pub fn new(db: &sled::Db) -> Self {
        let titles = ["99percentinvisible", "other_podcast"];
        let mut page = Podcasts {
            list: List::default(),
            search: Search::default(),
            podcasts: database::Podcasts::open(&db)
                .wrap_err("could not get list of subscribed podcasts")
                .unwrap(),
        };
        for title in titles.iter() {
            page.list.podcast_names.push(title.to_owned().to_string());
            page.list.podcast_buttons.push(button::State::new());
        }
        page
    }
    pub fn update(&mut self, message: Message) -> Command<crate::Message> {
        dbg!(&message);
        match message {
            Message::SearchSubmit => self.search.update(message),
            Message::SearchInputChanged(_) => self.search.update(message),
            Message::SearchResults(r) => {
                self.list.update_feedres(r);
                Command::none()
            }
            Message::AddPodcast(_) => {
                Command::none()
            }
        }
    }
    pub fn view(&mut self) -> Element<crate::Message> {
        let scrollable = self.list.view(&self.search.input_value);
        let searchbar = self.search.view();

        let column = Column::new()
            .push(searchbar)
            .push(scrollable);
        column.into()
    }
}
