# KyberNow service privacy policy

English · [Français](confidentialite.md)

> **Draft for review by a lawyer.** This text is not in force and must not be published as it stands. It describes what the relay actually keeps according to the specification ([comptes.en.md](../comptes.en.md), section "Data kept by the relay") and [relais.en.md](../relais.en.md); if the code changes, this text must change with it. Passages in brackets await information or a decision; the "For the lawyer" notes are removed from the published version. Version 0.2 of 25/11/2026. The French version prevails.

This policy explains which personal data the **KyberNow service** (accounts, the console and the public relay `share.kybernow.app`) processes, why, for how long, who sees it, and how to exercise your rights. It supplements the [terms of use](conditions.en.md).

The www.kybernow.app website and the software used without an account have their own rules, described in the website's legal notice: at home, on your network, the KyberNow software sends nothing to Les Lignes Claires.

## 1. The controller

**Matthieu Requenna, sole trader (entrepreneur individuel), trading as Les Lignes Claires**, SIREN 510 178 577, Bâtiment 1, appartement 3, 230 avenue du Maréchal Leclerc, 34070 Montpellier, France. `[TO CHECK at launch.]`

Contact about your data: **contact@leslignesclaires.fr** `[TO DECIDE: dedicated address, for example privacy@kybernow.app]`.

We have not appointed a data protection officer: the law does not require us to. The person who answers your questions is the publisher in person.

> **For the lawyer.** Confirm that appointing a DPO is not mandatory (GDPR article 37: no regular and systematic monitoring on a large scale?), given the organizations' access log.

**Organizations.** When a company, an association or a public body uses the service for its members and computers, **it** decides who has access to what and who reads its log: it is the controller of that data, and we process it on its behalf, as a processor, under a [data processing agreement](dpa.en.md). For that data, contact the organization first; we pass on to it any requests we receive.

## 2. The data we process

### 2.1 Your account

| Data | What it is for |
|---|---|
| Email address, name | Identifying you, writing to you (verification, forgotten password, new device, additions), finding you when someone adds you by email |
| Password hash (Argon2id) | Checking your password without ever keeping it in clear |
| Two-factor secret (encrypted), recovery code hashes | Two-factor authentication |
| Passkeys: identifier, public key, counter, name, dates | Signing in with a passkey (the private key stays on your device) |
| Dates: creation, verification, deletion requested; account plan | Managing the account |
| `[TO DECIDE: version of the terms accepted and date]` | Proving acceptance of the terms |

### 2.2 Your devices and sessions

| Data | What it is for |
|---|---|
| Connected devices: name ("Claire's iPad"), kind, last seen; sessions (token hash, dates, two-factor done) | Keeping your devices signed in, showing them to you, signing them out |
| Trusted browsers (hash, 30-day expiry) | Not asking for two-factor authentication again on that browser |
| Email tokens (hash, purpose, 24-hour or one-hour expiry) | Verifying your address, changing a forgotten password |

### 2.3 Your computers, rights and organizations

| Data | What it is for |
|---|---|
| Attached computers: Station identifier, name ("Living room PC"), owner (you or an organization) | Showing your computers in the apps, issuing access |
| Rights: who has which right (Session, Lending, Sharing, Administer) on which computer, expiry | Deciding on access |
| Organizations: name, members and roles, groups | Managing an organization's fleet |
| People added by email who do not have an account yet: email address, offered rights | Sending them the invitation and attaching the rights when they create their account |
| Access tokens issued (identifier, account, device, computer, rights; valid 5 minutes) | Opening a computer; preventing a token from being used twice |

### 2.4 The log

| Data | What it is for |
|---|---|
| Account security events: sign-in, failure, new device, two-factor changed, with date and IP address | Showing you your account's activity, detecting an intrusion |
| Access and rights log: who opened which computer, when, from which IP address and which device; every change of rights and by whom | Security; for an organization, knowing who accessed its computers |

### 2.5 Passing through the relay

The relay carries the connection between your devices and your computers. It sees:

- the **IP addresses** of devices and computers, connection times, and the volume and rate of data;
- **share codes**, for as long as it takes to put people in touch (never written to the logs);
- in the KyberNow Link apps, **nothing of what you see or type**: picture, sound, keyboard, mouse and controller are end-to-end encrypted between the app and the computer;
- in a **browser**, the relay decrypts the connection to pass it on to the computer; it keeps nothing of it.

### 2.6 What we do not have

No means of payment (the service is free), no audience measurement, no advertising, no tracking cookies, no sign-in through Apple, Google or any other third party. We do not see your screens, your files or your keystrokes.

### 2.7 Your messages to us

If you write to us: your address, your message and our reply.

## 3. Why, and on what legal basis

| Purpose | Legal basis (GDPR article 6) |
|---|---|
| Creating and keeping your account, signing you in, your devices, computers and rights, service emails | **Performance of the contract** (6.1.b): the terms of use |
| Managing organizations, their members, groups and log | On behalf of the organization, which is the controller ([data processing agreement](dpa.en.md)); for us, **performance of the contract** with it |
| Account and relay security: security log, new-device notices, attempt limits, technical logs with IP addresses, fighting abuse | **Legitimate interest** (6.1.f): protecting your accounts, your computers and the service |
| Answering your requests and the exercise of your rights | **Legal obligation** (6.1.c) for rights; **legitimate interest** for the rest |
| Keeping what the law requires, answering an authority that asks | **Legal obligation** (6.1.c) |
| `[Later: billing for the paid offer]` | Performance of the contract and legal obligation (accounting) |

We do not use your data for advertising, profiling or automated decisions about you. The only automatic decision is access itself: the relay issues a token if your account has the right, and limits excessive attempts.

## 4. How long

| Data | Period |
|---|---|
| Account (email, name, password, two-factor, passkeys) | Lifetime of the account |
| Connected devices and sessions | Lifetime of the device: until it is signed out or the session ends `[TO SPECIFY: lifetime of an inactive device session]` |
| Trusted browsers | 30 days |
| Email tokens | 24 hours (verification) or one hour (password) |
| Access tokens | 5 minutes; their identifier for as long as needed to prevent reuse |
| Attached computers | Until detached |
| Rights | Until removed or expired |
| Organizations, members, groups | Lifetime of the organization; a removed member disappears immediately |
| People added without an account | `[TO DECIDE: 30 days without creating an account, then erased]` |
| Log (security, access, rights) | **12 months** |
| Relay technical logs (IP addresses, connection times) | **A few days** (logs rotated by size) `[TO SPECIFY: maximum period in days]` |
| Share codes | For the duration of the share; never in the logs |
| Accounts never verified | `[TO DECIDE: 7 days]` |
| Inactive accounts | `[TO DECIDE: 3 years without sign-in, after two warning emails]` |
| Messages with us | `[TO DECIDE: 3 years after the last exchange]` |

**Deleted account**: all its data is erased **within 30 days, backups included**. Lines of an organization's log that concern it remain with the organization until the end of their 12-month period `[TO DECIDE: or anonymised when the account is deleted]`.

> **For the lawyer.** (1) The log keeps the IP address for 12 months, whereas the technical logs keep it for a few days: should the address be truncated or erased from the log sooner (for example after 3 months)? (2) Does decree no. 2021-1362 on retention of identification data apply to the service (access provider, host, neither)? If so, some data would have to be kept for a year after account deletion, which contradicts "within 30 days".

## 5. Who sees your data

- **Les Lignes Claires**: the publisher, currently the only person administering the relay.
- **The people you share with**: a computer's owner (or the Administrators of its organization) sees your name, email address, rights and access to that computer; they receive an email the first time one of your devices opens it; the Station keeps a local list of the accounts and devices that came in. An organization's Owners and Administrators see its members and its log.
- **Our processors**, each for its own task only:

| Processor | Task | Where |
|---|---|---|
| SendGrid, by Twilio Inc. | Sending the service's emails (recipient address, message content) | United States (see section 6) |
| `[TO COMPLETE: relay host or internet provider, if the relay is no longer on Les Lignes Claires' hardware]` | Hosting the relay | France |
| `[TO COMPLETE: backup storage, if external]` | Keeping encrypted backups of the database | `[TO COMPLETE]` |

- **Authorities** (police, courts, the CNIL) where the law requires us to answer them, on written request and within the limits of that request.

We do not sell or rent your data, and we give it to no one else.

## 6. Transfers outside the European Union

The relay is in France. The service's emails are sent through SendGrid, by Twilio Inc., in the United States: your email address and the message content are passed to it for sending, nothing else. Twilio is certified under the EU-U.S. Data Privacy Framework, recognised by the European Commission's adequacy decision of 10 July 2023; its data processing agreement also includes the Commission's standard contractual clauses, which would apply should that framework fall. No other data leaves the European Union. Should another transfer be added, this policy would say so beforehand, with the safeguard used.

When you open a computer from another country, the connection goes through the relay in France: it is you who route it there, not us.

## 7. Security

- Passwords kept as **Argon2id** hashes; at least 10 characters; known passwords refused.
- **Two-factor authentication** (authenticator app) and **passkeys**; two-factor authentication mandatory for administrative roles.
- Two-factor secrets **encrypted**; session tokens, email tokens and recovery codes kept as **hashes** only.
- Console: encrypted connection (HTTPS), site-bound cookies (`__Host-`, `Secure`, `HttpOnly`), protection against forged requests, **attempt limits** per address and per account; identical answers whether or not an account exists.
- Access to computers through **signed tokens valid for 5 minutes**, checked by the Station; on the Station, a switch, a list of access with blocking, and a notice for each new device.
- In the apps, **end-to-end encryption** between the app and the computer: the relay reads neither the picture nor the input.
- Email addresses and tokens **never written to the logs**; nor are share codes.
- Database backed up `[TO COMPLETE: frequency, encryption, location of backups]`; signing key in a file reserved for the relay.

No system is one hundred per cent secure. If you find a vulnerability, write to us `[TO DECIDE: reporting address, security.txt file]`.

## 8. In the event of a data breach

If a breach of your data happens (leak, loss, access by someone not entitled to it):

- we notify the **CNIL within 72 hours** of becoming aware of it, unless it presents no risk to you;
- if it presents a **high risk** to you (for example if access to your computers may have been granted wrongly), we tell you **without delay**, by email, saying what happened, its likely consequences, what we are doing and what you can do (change your password, turn off "Accept KyberNow accounts" in Admin, check the list of access);
- for an organization, we notify its Owners within the time set by the [data processing agreement](dpa.en.md);
- we keep a record of every breach, including those not notified.

## 9. Cookies

The console only sets cookies needed for the service, which do not require your consent:

| Cookie | What it is for | Duration |
|---|---|---|
| `__Host-kn_session` | Keeping you signed in | Session lifetime `[TO SPECIFY]` |
| Trusted browser | Not asking for two-factor authentication again | 30 days |
| Form protection | Preventing forged requests | Lifetime of the page |
| `__Host-kybernow_guest` (sharing in a browser) | Recognising you as a guest of a share | Duration of the share `[TO CHECK]` |

No audience measurement, advertising or social network cookies. The relay's pages load nothing from any other site.

## 10. Your rights

You have the right:

- to **access** your data and get a copy;
- to have it **corrected** (your name and address can be changed in the console);
- to have it **erased** ("Delete account" in the console);
- to **restrict** its processing, for example while we check a dispute;
- to **take it with you** in a machine-readable format ("Export my data" in the console);
- to **object** to processing based on our legitimate interest, on grounds relating to your situation (we may continue for compelling reasons, such as the security of other people's accounts);
- to give **instructions on what happens to your data after your death** (French law).

**How.** Most of these can be done in the console, without writing to us. Otherwise, write to the address in section 1, from your account's address. We answer **within one month** (three months for a complex request, in which case we tell you within the first month). We may ask you to prove that the account is yours, for example by signing in; we do not ask for identity documents without reason.

**An organization's data**: contact the organization; if you write to us, we pass your request on to it.

**Complaints.** If you think your rights are not being respected, you can complain to the **CNIL** (Commission nationale de l'informatique et des libertés, 3 place de Fontenoy, TSA 80715, 75334 Paris Cedex 07, France, www.cnil.fr), or to the authority of your country in the European Union.

## 11. Relays run by others

A KyberNow relay run by someone else (your company, an association) has its own accounts. Its data does not pass through Les Lignes Claires: whoever runs that relay is responsible for it and must inform you.

## 12. Changes

We update this policy when the service changes. For a significant change (new data, a new processor, a new purpose), we tell you by email and in the console before it applies. The date of the latest version is at the top of this text.
