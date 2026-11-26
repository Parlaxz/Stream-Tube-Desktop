# Changelog

## 0.11.4

### Tabs

- A closed group shows only its pill in the rail; the border is hidden.
- **New group** is a **+** button; the label shows only as a hover tooltip.
- The columns and sort pills are text-only, so the empty gap inside them is gone.
- Tab thumbnails use a 16:9 `mqdefault` image: the black bar at the top and the matching cut at the bottom are gone, and thumbnails have a rounded border.
- Home is removed from the tab grid and the non-video pills; the layout count excludes it.
- Dragging a tab over a group expands and highlights that group, and the target group flashes after the drop.

### Social

- The Social page has **Home**, **Room** and **Settings** tabs.
- **Home**: create or open a room, friends' rooms with Join, share what you are watching, shared-with-you and friends' activity.
- **Room**: enabled while in a room; shows the current video, the room queue with play/remove, **Open room tab** and **Leave**.
- **Settings**: relay, token and name, your friend code, add a friend, and the friend list with Invite/Send/Remove.

## 0.11.3

### Fixed

- The top-bar **Settings** button did nothing in 0.11.2. It now opens Settings in its own tab, or switches to the existing Settings tab, and uses the active tab's live route so a stale route cannot swallow the click.
- A tab's stored route stays current after in-tab navigation, so route-based tab lookups no longer match the wrong tab.

### Updates

- First delta update: 0.11.2 → 0.11.3 ships as a small exe patch, so 0.11.2 can update in place instead of downloading the full installer.

## 0.11.2

### Tabs

- **Tab groups** sit after Home in the rail as collapsible bordered strips that hold their member tabs. Drag a tab onto a group pill to file it; the pill highlights while you drag. **All** in the tab overview shows only ungrouped tabs, and a card fades when it changes group.
- Group names save on Enter, on leaving the name box, and on **Done**, and they survive a restart.
- The tab overview header puts the group pills and the layout/sort controls on one row, removing the empty area left of the badges. Columns cycle **Dynamic → 4 → 6 → 8**; sort cycles **Manual → Length → Recency → Views**.
- Non-video tabs appear as icon pills under the group row, with the name on hover.
- The drag ghost is centered on the pointer instead of hanging from the top of the card. The close X on tab thumbnails is translucent.

### Player

- A video keeps playing when you switch tabs or go Home. Starting another video pauses the previous one.
- **Volume boost** is a toggle that really raises the slider to **500%**; it now raises mpv's `volume-max` instead of only changing the UI.
- **Read ahead** and **Back cache** are text boxes with an **Apply** button.
- The top-bar **Settings** button opens Settings in its own tab (or the existing Settings tab) and keeps the video in picture-in-picture. The crash from opening Settings while watching is fixed.

### Rooms and social

- Rooms are advertised to friends. Join a friend's room from **Social → Friends' rooms** without an invite.
- A room opens in a tab right after Home and shows the current video and the room queue.
- **Share** is a menu now: **Copy link** or **Share with friends**.
- The Social page hides connection fields when connected, lists friends' rooms, and stays centered.
- Selectable titles and comments have a right-click **Copy selected text**. A link does not open when you have selected text.

### Updates

- Updates try a **bidiff delta of `streamtube.exe`** first: about 3.7 MB in testing instead of the 43 MB installer. The patch is checksum-verified, the new exe replaces the old one on restart, and the full installer remains the fallback. `release.bat` builds and uploads the patch.

## 0.11.1

### Tabs

- **Tab groups**: group pills sit at the top of the tab overview and in the tab rail. Drag a tab onto a pill to file it, drag onto **All** to unfile it. Click a rail pill to collapse or expand its tabs; right-click one to rename it, change its icon or color, or delete it. Groups and membership survive a restart.
- **Non-video tabs** (Home, Settings, Downloads, Social, channels, playlists…) appear as icon pills under the group row, with the name on hover.
- Tab overview layouts now cycle **Dynamic → 4 → 6 → 8** columns, and tabs can be sorted by **Length**, **Recency** or **Views**.

### Player

- A video keeps playing when you switch to another tab or Home. Starting another video pauses the previous one.
- **Volume boost** is a toggle now: it raises the volume slider's cap to **500%** and adds no audio gain.
- **Read ahead** and **Back cache** have sliders in the player's cog menu, with the presets still there.
- The top-bar **Settings** button opens Settings in its own tab (or the existing Settings tab) and keeps the video in picture-in-picture.

### Reading

- Video titles on cards and under the player are selectable: drag over the text, then copy it.

### Shorts

- Shorts show relative upload times (minutes, hours, days, weeks, months) and a plain date after a year, plus a **New** badge for anything uploaded in the last week.

## 0.11.0

### Watch party

- Rooms open in their **own tab** with their own player and queue. Videos you play in other tabs are no longer shared; only the room tab is synced.
- **Room queue**: add videos by pasting a link (**Add to queue** or **Play now**), click a queue row to play it, or remove a row from its menu.
- **Add to room** on any thumbnail (hover) and on the watch page beside Like/Share, while a room is open. The video menu also has **Add to room queue** and **Play in room**.
- Everyone in the room can pause, seek, change speed, pick the video and edit the queue; the host relays and keeps one clock.
- Invites arrive as a toast and open the room tab.

### Social

- The **Watch party** page is now **Social** in the sidebar. It keeps friends, presence, invites and room creation, and the layout is centered. Friend codes stay hidden until you press **Show friend code**.
- **Settings → Social**: choose what friends can see — **Likes** and **Dislikes**. Everything is off by default; activity goes only to your saved friends through your relay.
- The Social page shows **Shared with you** (videos a friend sent to you) and a **Friends' activity** feed of likes, dislikes and shared videos.
- The top-bar **bell** shows videos friends shared with you; opening it marks them read. The icon shows a dot while something is unread.
- **Share with friends** in any video's menu sends it to all friends. Each friend in Social has **Send**, which shares the video you are watching with that one friend.

### Tabs

- Close a tab from the tab overview with its **X** button or a middle-click on the card. The pinned Home tab stays open.
- Click a channel name in the tab overview to open the channel. Middle-click or Ctrl/Cmd+click opens it in a new background tab.
- Tab overview cards respond on hover and fade out when closed. The overview stays open after a tab closes.
- Background tabs keep channel names and links, including after a restart.

## 0.10.1

### Fixed

- Playback works again: local HLS URLs now carry the proxy's session token. 0.10.0 refused every video with a 403.

### Player

- **Volume boost** in the player's cog menu: **Off**, **+50%** or **+100%**. It applies to the current video only; the next video starts at normal volume.
- **Read ahead** and **Back cache** are now in the cog menu too, with presets from 48 to 768 MB and 8 to 256 MB. They apply to every open player immediately, like the same boxes in Settings → Custom Features.

## 0.10.0

### Upstream sync

- Updated to upstream `bdcab25` (196 commits). Fork features win wherever the two overlap; the rest of the upstream work is included.
- Kept the fork's tab and player model (a player per tab, pinned Home tab) instead of upstream's shared-player tabs.

### Player

- Aspect ratio menu in the player, plus `Z` to cycle: **Auto** finds and crops a film's black bars for 21:9 and 32:9 screens, or pick 21:9, 18:9, 32:9, *Zoom to fill* or *Stretch*.
- Step one frame at a time with `,` / `.` while paused, or **Previous frame** / **Next frame** in the right-click menu.
- The volume bar shows its level as a number; **Settings → Player → Hide volume level** hides it.
- In theater mode, the wheel over the right edge of the player scrolls the page instead of changing the volume.
- Playback keeps the display awake on supported systems, and a stalled decoder (no new frames after a seek) now recovers or reports the error.
- Buffered ranges stay visible after seeking backwards.

### Tabs

- Home is a pinned first tab: icon only, always first, never closed, dragged or navigated away. Opening Home or another page from it opens a new tab.
- Drag tabs to reorder them, in the tab rail and in the tab grid.

### Cards, channels and shorts

- Middle-click or Ctrl+click a channel name or avatar to open the channel in a new background tab; hovering the avatar shows a ring.
- Shorts cards show views and upload date in one line.
- Hover a video title to read it in full.

### Downloads

- Download a transcript as plain text (`.txt`) or timestamped subtitles (`.srt`) from the Download menu. With several caption tracks, pick the language; automatically generated tracks are labelled.

### Languages

- **Settings → General → App language** offers English, Azerbaijani, French, German, Japanese, Kazakh, Russian, Turkish, Ukrainian, Simplified Chinese and Traditional Chinese. YouTube and caption languages follow it until you choose them separately.

### Updates and packaging

- Portable Windows builds can update in place, keeping settings and history.
- The RPM package now requires the libmpv SONAME the app actually links.
- A second launch hands its link to the running instance; the handoff is token-checked.

### Fixes

- Hover previews are positioned when a card is first hovered.
- Slow downloads continue while data arrives.
- Translation fixes for comment reply counts, live chat status and playlist cleanup notices.

## 0.9.0

### Custom Features

- New **Settings → Custom Features** section, between *Player* and *Premium features*.
- Configurable playback speed shortcuts, all respected at runtime:
  - **Increase speed** — default key `D`, with its own step amount (default `0.25`).
  - **Decrease speed** — default key `S`, with its own step amount (default `0.25`).
  - **Reset speed** — default key `R`, returns to `1.00x`.
  - **Default speed shortcut** — default key `G`, applies the configured default speed.
- **Default speed** box: sets the speed videos start at and what the `G` shortcut applies.
- Speed control in the playback bar: current speed between `−` and `+` buttons, placed left of the subtitles button behind a vertical divider. The `−`/`+` buttons use the configured step amounts, and clicking the speed opens the speed menu.
- Each shortcut press shows the same speed popup as before.
- **Read ahead** card: choose the forward and back video buffer sizes in MiB. Each open video tab keeps its own buffer in RAM.
- **App icon** card: pick the window, taskbar and in-app icon from the 21 ReVanced branding icons (AFN, MMT, Revancify, Vanced, Xisr and more), or keep **Default**. Icons show in a grid; the selected one is outlined and applied with **Save**.
- The chosen icon is used for the window and taskbar, desktop/Start/taskbar shortcuts, the Linux menu icon, and the logo inside the app.
- The top bar now shows only the chosen app icon (still opens Home) instead of the "Stream Tube" wordmark.

### Tabs & Navigation

- **Middle-click** a video or short — on home, subscriptions, search results, or the related/queue list — to open it in a new background tab, same as **Ctrl/Cmd+click**.
- **Ctrl/Cmd+click** or **middle-click** any sidebar item (Home, Shorts, Subscriptions, History, Playlists, Watch later, Downloads, channels, Explore, Settings) to open it in a new tab. New sidebar items get this automatically.
- Right-click a video thumbnail opens **Save to playlist**; middle-click now opens the video in a new tab instead of opening that dialog.
- Each tab now has its own player. Switching away from a video tab pauses it but keeps its engine, demuxer cache and exact position in RAM; switching back resumes it without downloading the loaded part again. Closing a tab frees its buffer.
- Opening a video in a background tab loads nothing until the tab is clicked. The tab shows the card title and channel icon only.
- Tabs persist across a full restart: routes, titles, channel icons and the active tab are restored. Video buffers cannot survive a process restart.
- The tab bar has a grid button, and **left+right click anywhere** opens the same tab overview. The grid fills 90% of the window in both directions and is scrollable.
- The grid shows video thumbnails (icons for other pages), with the channel avatar under each video thumbnail. Click a tile to switch to that tab.
- Hovering a video tab in the tab bar shows its thumbnail.
- The grid uses **6 columns** by default. The **Dynamic** toggle switches to a square-ish layout (`rows = floor(sqrt(n))`, `columns = ceil(n / rows)`). The choice is remembered.
- The 12-tab limit is removed.


### Home Page preservation

- The top-bar **pin** keeps the home feed through reloads: while it is on, a refresh keeps the page and scrolling only appends new videos.
- **Settings → Custom Features → Home Page preservation** chooses the mode:
  - **Pin button** — the top-bar pin decides (default).
  - **On hard refresh and close** — the feed is kept, but a manual refresh or an app restart reloads it.
  - **Off** — reload with the page.

### Subscription profiles

- Profiles group subscribed channels into separate feeds. Create them under **Subscriptions → Manage**; nothing is uploaded.
- Profiles appear in the **Subscriptions** feed and in the sidebar. Clicking a sidebar profile opens its feed and refreshes it.
- The sidebar **Subscriptions** section collapses with a chevron, and the collapsed state is kept between launches.
- Pinned playlists now sit above **Subscriptions** in the sidebar.
- **Settings → Custom Features → Subscription profiles** adds a **Refresh time** choice (default: 1 hour). A profile's feed reloads when its cache is older than that.
- **Import profiles** from a PocketTube subscriptions CSV (`Title, Link, Subscribers, Groups`). Grouped channels are subscribed and added to their profiles.

### Player & Scrolling

- **Scroll wheel over the video** changes the volume by 3% per notch. It works while the video is paused and in fullscreen. The up/down arrow keys still change the volume by 5%.
- **Middle-click empty space** on a page starts browser-style autoscroll. A middle click on a video still opens it in a new tab instead. Move the pointer up or down to scroll. The speed follows the distance from the click and has no upper limit. A marker shows where the click was. Another middle click, any other click, or leaving the window stops it.

### Thumbnails

- The `⋮` menu on video cards and the watch page now has **Copy thumbnail**, below **Copy link**. It copies the thumbnail image itself to the clipboard.
- Copied images also use the standard Windows `CF_DIB` format, so they paste into Explorer, Paint, .NET apps and clipboard history.

### Playlists

- Pin a YouTube playlist with the pin button on its card in **Playlists**; pinned playlists show in the sidebar.
- The sidebar lists only pinned playlists. A **Show N more** row reveals the unpinned ones.
- Hovering a video thumbnail shows **Watch later** and **Add to queue** buttons.
- **Save to playlist** lists local and YouTube playlists with checkboxes. The checkboxes show which playlists already contain the video, and those playlists are listed first.
- Left-click a playlist to save and close the dialog. Right-click its checkbox to toggle it without closing.
