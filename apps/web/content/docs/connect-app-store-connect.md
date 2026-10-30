---
title: Connect App Store Connect
description: Create an App Store Connect API key so AppDoctor can read your keyword field and update your listing.
order: 2
group: Connect your accounts
---

Your public App Store listing doesn't show your keyword field. To see it, and to send listing changes back to Apple, AppDoctor needs an App Store Connect API key. The key stays on your computer, in the macOS Keychain or Windows Credential Manager, and is only ever sent to Apple.

## Create the key

1. Sign in to [App Store Connect](https://appstoreconnect.apple.com) as the Account Holder or an Admin.
2. Go to **Users and Access**, then **Integrations**, then **App Store Connect API**.
3. Under **Team Keys**, create a key. Name it "AppDoctor" and give it the **App Manager** role.
4. Download the `.p8` file. Apple only lets you download it once, so keep it somewhere safe.
5. Note the **Issuer ID** (at the top of the page) and the key's **Key ID**.

## Add it to AppDoctor

In AppDoctor, open **Settings → App Store Connect**, choose the `.p8` file, and enter the Issuer ID and Key ID.

## What AppDoctor does with it

- Reads your app's name, subtitle, keyword field and description in each language.
- Saves your edits to a version that is still being prepared.
- Submits for review only when you choose **Submit for review** and confirm the summary it shows. This includes when an AI agent asks to submit.

To revoke access, delete the key in App Store Connect. AppDoctor stops working with it immediately.
