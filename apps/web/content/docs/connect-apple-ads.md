---
title: Connect Apple Ads
description: Connect your Apple Ads account to see Apple's 0–100 search popularity for any keyword.
order: 3
group: Connect your accounts
---

Apple Ads publishes how often a search phrase is used, as a popularity score from 0 to 100. AppDoctor reads it through the Apple Ads Platform API with your own Apple Ads account, directly from your computer.

## Before you start

You need an [Apple Ads](https://ads.apple.com) account. Use the **Advanced** sign-in. Your user needs an API role (API Account Manager, API Read Only or Limited Access); an account administrator can grant one under **Account Settings → User Management**.

## Connect

1. In AppDoctor, open **Settings → Apple Ads** and choose **Create key**. AppDoctor creates a key pair and shows you the public key. The private key never leaves your computer.
2. Copy the public key.
3. In Apple Ads, go to **Account Settings → API**, paste it into **Public Key**, and save.
4. Apple shows a `clientId`, `teamId` and `keyId`. Copy them into AppDoctor and choose **Connect**.

AppDoctor signs a short-lived token with your key whenever it needs one. To disconnect, remove the public key in Apple Ads.

## What you get

- **Popularity** for every keyword you track, from Apple.
- **Keyword ideas** that Apple associates with your app.
- **Trending searches** in your app's category, by country, week by week.
