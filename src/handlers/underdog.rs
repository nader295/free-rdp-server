use rust_i18n::t;
use teloxide::Bot;
use teloxide::payloads::AnswerCallbackQuerySetters;
use teloxide::requests::Requester;
use teloxide::types::{CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup, UserId};
use crate::domain::LanguageCode;
use crate::handlers::utils::callbacks::{CallbackDataWithPrefix, InvalidCallbackData, InvalidCallbackDataBuilder};
use crate::handlers::{CallbackResult, HandlerResult};
use crate::repo::Repositories;

/// Underdog support configuration
pub struct UnderdogConfig {
    /// Number of consecutive losses to trigger comeback boost
    pub comeback_boost_threshold: u32,
    /// Number of weekly losses to give phoenix seed
    pub phoenix_seed_threshold: u32,
    /// Mentor bonus percentage
    pub mentor_bonus_percent: f32,
}

impl Default for UnderdogConfig {
    fn default() -> Self {
        Self {
            comeback_boost_threshold: 3,
            phoenix_seed_threshold: 5,
            mentor_bonus_percent: 0.05, // +5%
        }
    }
}

/// Callback data for mentor acceptance
#[derive(derive_more::Display)]
#[display("{action}:{mentor_id}:{mentee_id}")]
pub struct MentorCallbackData {
    action: MentorAction,
    mentor_id: UserId,
    mentee_id: UserId,
}

#[derive(Clone, Copy, Debug)]
pub enum MentorAction {
    Accept,
    Decline,
}

impl MentorAction {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "accept" => Some(Self::Accept),
            "decline" => Some(Self::Decline),
            _ => None,
        }
    }
    
    fn as_str(&self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Decline => "decline",
        }
    }
}

impl std::fmt::Display for MentorAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl MentorCallbackData {
    pub fn new(action: MentorAction, mentor_id: UserId, mentee_id: UserId) -> Self {
        Self { action, mentor_id, mentee_id }
    }
}

impl CallbackDataWithPrefix for MentorCallbackData {
    fn prefix() -> &'static str {
        "mentor"
    }
}

impl TryFrom<String> for MentorCallbackData {
    type Error = InvalidCallbackData;

    fn try_from(data: String) -> Result<Self, Self::Error> {
        let err = InvalidCallbackDataBuilder(&data);
        let mut parts = data.split(':');
        
        let action_str: String = crate::handlers::utils::callbacks::parse_part(&mut parts, &err, "action")?;
        let action = MentorAction::from_str(&action_str).ok_or_else(|| err.split_err())?;
        let mentor_id = crate::handlers::utils::callbacks::parse_part(&mut parts, &err, "mentor_id").map(UserId)?;
        let mentee_id = crate::handlers::utils::callbacks::parse_part(&mut parts, &err, "mentee_id").map(UserId)?;
        
        Ok(Self { action, mentor_id, mentee_id })
    }
}

/// Record a battle loss and check for underdog support triggers
pub async fn record_loss(
    repos: &Repositories,
    user_id: UserId,
    chat_id: ChatId,
    config: &UnderdogConfig,
) -> anyhow::Result<Option<UnderdogSupport>> {
    let user_id_i64 = user_id.0 as i64;
    let chat_id_i64 = chat_id.0;
    
    // Get current underdog stats
    let stats = get_underdog_stats(repos, user_id_i64, chat_id_i64).await?;
    
    let consecutive_losses = stats.consecutive_losses + 1;
    let weekly_losses = stats.weekly_losses + 1;
    
    // Check for comeback boost (3 consecutive losses)
    if consecutive_losses >= config.comeback_boost_threshold && !stats.comeback_boost_active {
        // Activate comeback boost
        // update_underdog_stats(repos, user_id_i64, chat_id_i64, ...).await?;
        return Ok(Some(UnderdogSupport::ComebackBoost));
    }
    
    // Check for phoenix seed (5 weekly losses)
    if weekly_losses >= config.phoenix_seed_threshold && !stats.phoenix_seed_given {
        // Give phoenix seed
        // update_underdog_stats(repos, user_id_i64, chat_id_i64, ...).await?;
        return Ok(Some(UnderdogSupport::PhoenixSeed));
    }
    
    Ok(None)
}

/// Record a battle win (resets consecutive losses)
pub async fn record_win(
    _repos: &Repositories,
    _user_id: UserId,
    _chat_id: ChatId,
) -> anyhow::Result<()> {
    // Reset consecutive losses to 0
    Ok(())
}

/// Types of underdog support that can be triggered
#[derive(Clone, Debug)]
pub enum UnderdogSupport {
    ComebackBoost,
    PhoenixSeed,
    MentorOffer { mentor_id: UserId, mentor_name: String },
}

impl UnderdogSupport {
    pub fn to_message(&self, lang_code: &LanguageCode) -> String {
        match self {
            Self::ComebackBoost => {
                t!("commands.underdog.comeback_boost", locale = lang_code).to_string()
            }
            Self::PhoenixSeed => {
                t!("commands.underdog.phoenix_seed", locale = lang_code).to_string()
            }
            Self::MentorOffer { mentor_name, .. } => {
                t!("commands.underdog.mentor_offer", locale = lang_code, mentor_name = mentor_name).to_string()
            }
        }
    }
    
    pub fn keyboard(&self, mentee_id: UserId, lang_code: &LanguageCode) -> Option<InlineKeyboardMarkup> {
        match self {
            Self::MentorOffer { mentor_id, .. } => {
                let accept_btn = InlineKeyboardButton::callback(
                    "Accept",
                    MentorCallbackData::new(MentorAction::Accept, *mentor_id, mentee_id).to_data_string(),
                );
                let decline_btn = InlineKeyboardButton::callback(
                    "Decline",
                    MentorCallbackData::new(MentorAction::Decline, *mentor_id, mentee_id).to_data_string(),
                );
                Some(InlineKeyboardMarkup::new(vec![vec![accept_btn, decline_btn]]))
            }
            _ => None,
        }
    }
}

/// Underdog statistics from database
#[derive(Default)]
struct UnderdogStats {
    consecutive_losses: u32,
    weekly_losses: u32,
    comeback_boost_active: bool,
    phoenix_seed_given: bool,
}

/// Get underdog stats for a user in a chat
async fn get_underdog_stats(
    _repos: &Repositories,
    _user_id: i64,
    _chat_id: i64,
) -> anyhow::Result<UnderdogStats> {
    // In production, query the database
    Ok(UnderdogStats::default())
}

/// Filter for mentor callbacks
#[inline]
pub fn callback_filter(query: CallbackQuery) -> bool {
    MentorCallbackData::check_prefix(query)
}

/// Handle mentor acceptance callback
pub async fn callback_handler(
    bot: Bot,
    query: CallbackQuery,
    repos: Repositories,
) -> HandlerResult {
    let lang_code = LanguageCode::from_user(&query.from);
    let callback_data = MentorCallbackData::parse(&query)?;
    
    // Only the mentee can accept/decline
    if callback_data.mentee_id != query.from.id {
        bot.answer_callback_query(&query.id)
            .text(t!("inline.callback.errors.another_user", locale = &lang_code))
            .show_alert(true)
            .await?;
        return Ok(());
    }
    
    match callback_data.action {
        MentorAction::Accept => {
            // Get mentor info
            let mentor = repos.users.get(callback_data.mentor_id).await?
                .ok_or("mentor not found")?;
            
            // Create mentorship (in production)
            // repos.mentorships.create(mentor_id, mentee_id, chat_id).await?;
            
            let text = t!("commands.underdog.mentor_accept", locale = &lang_code, 
                mentor_name = mentor.name.to_string());
            
            CallbackResult::EditMessage(text.to_string(), None).apply(bot, query).await?;
        }
        MentorAction::Decline => {
            // Just remove the keyboard
            CallbackResult::EditMessage("Mentorship declined.".to_string(), None).apply(bot, query).await?;
        }
    }
    
    Ok(())
}

/// Find a potential mentor for an underdog
pub async fn find_mentor(
    _repos: &Repositories,
    _mentee_id: UserId,
    _chat_id: ChatId,
) -> anyhow::Result<Option<(UserId, String)>> {
    // Find a player in the top 25% who has been active recently
    // and isn't already mentoring someone in this chat
    Ok(None)
}
