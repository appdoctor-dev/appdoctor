---
title: Apple's new Ads API gives ASO tools real keyword popularity
description: The Apple Ads Platform API returns Apple's 0–100 popularity score for search phrases you choose. Here's what changed and what it means for keyword research.
date: 2026-10-29
author: AppDoctor
---

For years, ASO keyword research tools have had to estimate how popular an App Store search is. Some scraped numbers from the Apple Ads web interface; others guessed from the ratings of the apps that rank for it. In August 2026, Apple released the **Apple Ads Platform API**, and it changes that.

## What's new

The Apple Ads Platform API replaces the old Campaign Management API, which Apple will switch off on January 26, 2027. Alongside campaigns and reports, it has two parts that matter for keyword research:

- **Phrase popularity.** You send a list of search phrases and get back Apple's popularity score for each, from 0 to 100. This works for phrases you pick, not just ones Apple suggests.
- **Search term popularity.** A ranked list of the most-searched terms in each App Store category and country, week by week or month by month. It's good for spotting trends, but it only covers the top terms in each category.

Both use the same account and the same kind of key as any other Apple Ads API call. Apple's documentation describes them under "Suggestions" and "Insights".

## Why it matters

Popularity is the number every keyword decision rests on. With an official source:

- Numbers come from Apple, not from a model's estimate.
- An ASO tool can read them with *your* Apple Ads account, from your own computer, instead of sending your keyword list to a third-party server.
- New keyword ideas come with a popularity score attached.

## How AppDoctor uses it

AppDoctor connects to your Apple Ads account with a key created on your computer. Every keyword you track shows Apple's popularity score next to its difficulty and your rank in each country. See [Connect Apple Ads](/docs/connect-apple-ads) for the setup.
