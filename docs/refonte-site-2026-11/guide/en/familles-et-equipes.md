---
slug: familles-et-equipes
slug_en: families-and-teams
titre: Families and teams
title: Manage a family or a team · KyberNow
description: Bring your family or team together, give out roles, open groups of computers, follow the log and deploy Stations in bulk.
rubrique: Comptes et équipes
rubrique_en: Accounts and teams
univers: commun
duree: 15 min
connexes: [compte-kybernow, installer-station-windows, hors-de-chez-vous, installer-votre-relais]
---

A family or a team brings people and computers together. You open a computer, or a whole group of computers, to its members in one go, and the log keeps track of every access.

## Before you start

- A [KyberNow account](/guide/compte-kybernow/) protected by a passkey or by two-factor authentication: you need one to create a family or a team.
- The computers to share, with KyberNow Station installed.

## Family or team

A family and a team work the same way, with the same roles, rights and groups: “The Martins” on one side, “The studio” on the other. A team also has the “Deploy” tab, for installing Stations in bulk.

1. On the account page, open “Families and teams”, then “Create a family or team”.
2. Choose one or the other and give it a name. You become its Owner.
3. The family's or team's page has its tabs: “Members”, “Groups”, “Computers”, “Log”, and “Deploy” for a team.

<!-- capture : a team's page, “Members” tab -->

## The roles

| Role | What it can do |
|---|---|
| Owner | Everything: members, roles, groups, computers and the log. Only an Owner can appoint or remove another Owner, and take a computer out of the team. |
| Administrator | Invites and removes members, creates groups, attaches computers, opens them to members, reads the log and, in a team, creates enrolment tokens. |
| Member | Uses the computers opened to them, with the rights given. |

Owners and Administrators have every right on the family's or team's computers. They must use two-factor authentication or a passkey: without it, they only have a member's rights. A family or team always keeps at least one Owner.

## Invite the members

1. “Members” tab, “Invite a member”.
2. Type the email address and choose the role.
3. If the person has an account, they find the invitation under “Invitations”, on their account page, and accept it. If not, the email offers to create one; the invitation waits for them.

An invitation expires after 30 days. “Resend the invitation” extends it for another 30 days, at most once an hour.

## Groups of people

A group brings together members with the same needs: “Designers”, “Editing”, “The kids”.

1. “Groups” tab, “Create a group”, then add members to it.
2. On a computer's page, “Open to a family or team”: choose the whole family or team, or one of its groups, then the rights.

The rights are the same as for an account: “Watch”, “Open the desktop”, “Borrow to play”, “Join your shares” and “Administer” (see [Your KyberNow account](/guide/compte-kybernow/)). A member who was given “Administer” without being an Owner or Administrator can only give the rights they hold themselves.

## Computer groups

A computer group brings together machines you want to open together: “Studio workstations”, “Games room PCs”.

1. “Computers” tab, “Create a computer group”, then add computers from the family or team to it.
2. “Open the group”: to the whole family or team, to a group of people or to one member, with the rights and, if you like, an end date.

These rights apply to every computer in the group, today's and the ones you add later. They come on top of the rights given computer by computer. On a computer's page, “Who has access” shows them under “Through the group” followed by its name; you change them on the group. A computer can belong to several groups. “Delete the group” also removes the rights given on it.

## The family's or team's computers

- **A new computer**: when you attach it, in Admin, under KyberNow account, choose the family or team as owner instead of “Me”.
- **A computer that is already yours**: on its page, “Move…” it to the family or team (see [Your KyberNow account](/guide/compte-kybernow/)).
- **A computer that stays yours**: “Open to a family or team” shares it without changing its owner.

## The log

The “Log” tab says who opened which computer, when, with which device and from which address. It also records every change: members, roles, invitations, groups, rights given and removed, computers attached, detached or deployed, tokens, new devices, Remote Admin openings. IP addresses are forgotten after 7 days, entries after 12 months. Only Owners and Administrators can read it.

## Company sign-in (SSO and OAuth)

Your team can sign in with your company's directory, through SSO or OAuth: everyone gets in with their usual work account. An Owner or an Administrator sets it up in the team's console.

## Deploy Stations (teams)

To install the Station on dozens of machines, with no screens and no clicks, the team hands out an enrolment token: every Station installed with it attaches itself to the team.

### Create the token

1. “Deploy” tab, “New token”.
2. Give a “Name”, a “Duration (days)” from 1 to 365 days and a “Number of Stations” from 1 to 10,000.
3. Choose the “Group that gets the rights” and the “Rights given to the group” on each enrolled Station. To file the machines straight into a computer group, choose it in “File in the group”.
4. “Create the token”. It starts with `kne_` and is “Shown only once: copy it now.” One click selects it.

### Install on Windows

Run the installer as administrator, with the token and the relay's address (share.kybernow.app, or your own):

```
KyberNow-Station-Setup.exe /quiet /enroll=<token> /relay=<relay>
```

The Station installs with no screens, with the default options, and keeps the settings of an existing installation. The exit code tells you how it went:

| Code | Meaning |
|---|---|
| 0 | Installed and attached to the team |
| 3010 | Installed, the computer must restart |
| 1 | Installation failed |
| 2 | Installed, but not enrolled |

The details are written to `install.log`. The silent installation creates the `kybernow` Admin account with a random password: to take back control of a machine, choose “Reset the Admin password…” in the menu of the KyberNow Station icon, next to the clock.

### Install on a Mac

In the user's session and without `sudo`, run the `kybernow-ui` program from “Install KyberNow Station”, in the KyberNow-Station-Mac.dmg file:

```
kybernow-ui --quiet --enroll=<token> --relay=<relay>
```

macOS asks once for the password of an administrator of the Mac. Permissions (screen, accessibility, microphone) are then granted from the Station's icon, or through a configuration profile from your Mac management tool.

> Good to know: a deployment tool can also write the token into the Station's settings, `[account] enroll = "kne_…"` in `kyber-admin.toml`, with the relay under `[relay]`. Admin presents it as soon as the relay answers, then erases it.

### Follow and stop a token

The list of tokens shows, for each one, how many Stations it has enrolled, and whether it is “Expired”, “Used up” or “Revoked”. “Revoke the token” stops any new enrolment; Stations already enrolled stay in the team.

## Open Admin from the account page

If you administer a computer, “Open Admin” is on its page, on its card (the “Admin” shortcut) and on its row in the team's “Computers” tab. The page asks whether to open that computer's Admin: choose “Confirm with a passkey”. Admin opens already signed in, without its password.

At home, on the same network as the computer, this works straight away. Admin's sign-in page also offers “Sign in with KyberNow”, on the network only.

### Remote Admin

To open Admin from anywhere, first switch on “Remote Admin”:

1. On site, on the computer's network, open Admin, Link devices › “Away from home”.
2. Check that the computer is attached to an account, with “Accept KyberNow accounts”, and that “Access away from home” is on.
3. Switch on “Remote Admin”. It is off to begin with and can only be switched on from here; it can be switched off from anywhere.
4. Add a passkey to your account if you haven't already: it is asked for each time Admin is opened remotely.

Remotely, all of Admin is there, with the same pages and the same settings. Only “Remote Admin” itself cannot be switched on from there.

Every remote opening is visible. The Mac shows a notification and the Windows icon opens its panel, with “Admin is open remotely by” followed by the name. Admin shows the banner “Admin opened remotely, through KyberNow Share.” Admin's feed and the team's log record it, and the Owners and Administrators receive an email. The session closes after 30 minutes without activity, and after 8 hours in any case.

Under the switch, Admin says whether the connection is “Encrypted end to end, all the way to this computer: the relay does not see what goes through.” With a relay that doesn't allow this, Admin says so too: that relay then sees what goes through.

## If something goes wrong

- **You can't create a family or a team.** First turn on two-factor authentication, or add a passkey, under “Account and security”.
- **“You don't manage” the chosen family or team.** You must be an Owner or Administrator there to move a computer into it.
- **The silent installation returns code 2.** The token has expired, is used up or was revoked, or the relay has no accounts. Look at the “Deploy” tab and at `install.log`.
- **“Remote Admin is off” on this computer.** The link only works at home: choose “Open on the network”, or switch on “Remote Admin” on site.
- **“Open Admin” is greyed out, with “Offline”.** The computer is off, asleep or without internet.
- **“Add a passkey to your account first”** appears. Add one under “Account and security”, then try again.
