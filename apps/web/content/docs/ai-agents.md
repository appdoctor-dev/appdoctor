---
title: Use AppDoctor with AI
description: Let Claude Code, Codex, Gemini CLI or a model in Ollama work on your keywords, listing, screenshots and videos, inside AppDoctor or from Claude, Cursor and other MCP apps.
order: 8
group: AI
updated: 2026-10-30
---

AppDoctor works with the AI you already have. There's no second AI subscription and nothing new to sign up for: it uses the sign-in your AI tools already have on your computer, or a model running in Ollama.

There are two ways to use it.

## Inside AppDoctor

Click **AI** in the header of **Screenshots** or **Localization**. A chat opens beside the screen you're on.

1. Under the message box, choose the **project folder** with your app's code. The AI reads it to learn what your app really does, and runs in it, so your project's `CLAUDE.md` or `AGENTS.md` applies. AppDoctor suggests a folder when it finds one named after your app.
2. Choose the **AI and its model**. AppDoctor finds Claude Code, Codex (including the copy inside the ChatGPT app), Gemini CLI and Ollama with its models.
3. Choose the **effort**, for models that offer it: more effort is slower but more careful.
4. Type what you want, or pick a suggestion, and press Return.

The chat shows what the AI is doing in plain words ("Read your listing", "Drew a picture"), and you see each change on the canvas as it's made. ⌘Z undoes it. Follow-up messages carry on the same conversation.

## From Claude, Cursor and other apps

AppDoctor includes an MCP server, so AI apps on your computer can use its tools. In **Screenshots**, click **AI agent**: AppDoctor lists the AI apps it finds (Claude, Claude Code, Cursor, VS Code, Windsurf, Codex, Gemini CLI, Antigravity and LM Studio) and **Connect all** adds AppDoctor to each one's settings. It keeps everything else in those files and saves a copy first.

To add it by hand, the server is the AppDoctor app itself, run with `mcp`:

```json
{
  "mcpServers": {
    "appdoctor": {
      "command": "/Applications/AppDoctor.app/Contents/MacOS/appdoctor",
      "args": ["mcp"]
    }
  }
}
```

It speaks MCP over stdio and opens no network port. It only reads and writes AppDoctor's own files on your computer.

## What the AI can do

- **Keywords and listing:** read the keywords you track with their ranks, popularity and difficulty in each country, read your App Store listing in every language, run an [ASO audit](/docs/aso-audit), and suggest new listing text.
- **Screenshots:** add slides and layers, set backgrounds and themes, place any iPhone or iPad, and draw illustrations as SVG that come in as separate layers you can move and edit.
- **Videos:** turn a design into an app preview, add camera moves, callouts and sound, look at a storyboard and check it before exporting.
- **The iOS Simulator:** take screenshots and screen recordings of your app to put in device frames.

## What it can't do without you

- Listing suggestions never reach the App Store by themselves. They wait in **Localization**, marked with a sparkle. **Review** puts the text in the fields to edit, and **Review and Save…** sends it with your App Store Connect key after a confirmation that lists every change.
- Submitting for App Review is done in **App Review**, after a summary of the app, version and build that you confirm.

## Things to ask

- "Improve my listing for the keywords I track."
- "Which tracked keywords is my listing missing, and what would you change?"
- "Write listings for the other countries I track."
- "Make a 20-second app preview of the app's best features."
- "Draw an illustration of what the app does on slide 1."
