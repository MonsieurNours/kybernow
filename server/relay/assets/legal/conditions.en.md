# KyberNow service terms of use

English · [Français](conditions.md)

> **Draft for review by a lawyer.** This text is not in force and must not be published as it stands. It was written from the accounts specification ([comptes.en.md](../comptes.en.md)) and from what the relay actually does ([relais.en.md](../relais.en.md)). Passages in brackets (`[TO COMPLETE]`, `[TO DECIDE]`) await information or a decision; the "For the lawyer" notes raise questions to settle and are removed from the published version. Version 0.1 of 23/11/2026. The French version prevails.

## 1. Who we are

The KyberNow service is provided by **Matthieu Requenna, sole trader (entrepreneur individuel), trading as Les Lignes Claires** ("Les Lignes Claires", "we").

- SIREN: 510 178 577 (SIRET 510 178 577 00019) `[TO CHECK: legal identity at launch, and whether a company replaces it]`
- Address: Bâtiment 1, appartement 3, 230 avenue du Maréchal Leclerc, 34070 Montpellier, France `[TO CHECK]`
- Contact: contact@leslignesclaires.fr `[TO DECIDE: dedicated address for the service, for example support@kybernow.app]`

## 2. What these terms cover

These terms apply to the **KyberNow service** provided by Les Lignes Claires, that is:

- **KyberNow accounts** created on the public relay `share.kybernow.app` (sign-up, sign-in, security, connected devices);
- the relay's **web console** (`https://share.kybernow.app/compte`), where you manage your account, your computers, the people you open them to and your organizations;
- the **public relay** `share.kybernow.app`, which carries the connection between your devices and your computers when they are not on the same network, and which is used for sharing (KyberNow Share);
- the **emails** the service sends you.

They do not apply to:

- the **KyberNow software** itself (KyberNow Station, Admin, the Link apps, the self-hosted relay), which is free software released under the AGPL-3.0 licence and governed by that licence. You can use it without an account, on your network, by pairing or by invitation;
- **relays run by others** (a company, an association, an individual): each relay has its own accounts and its own rules, set by whoever runs it. Les Lignes Claires have no access to them and are not answerable for them;
- **app stores** (App Store, Google Play, Microsoft Store…), whose terms also apply when you download an app there.

How we handle your personal data is described in the [privacy policy](confidentialite.en.md), which forms part of these terms.

## 3. Accepting these terms

You accept these terms by creating your account (the tick box on the "Create an account" page). If you do not accept them, do not create an account: the KyberNow software remains usable without one.

You must be at least **`[TO DECIDE: 15]`** years old to create an account. Below that age, a parent or guardian creates and holds the account.

> **For the lawyer.** Minimum age: the legal basis for the account is the contract, not consent, but can a minor enter into this contract alone (everyday acts, article 1148 of the French Civil Code)? Is a minimum age needed, and which one?

If you create an account or an organization for a company, an association or a public body, you declare that you have authority to bind it; these terms then also apply to it. For an organization that entrusts us with its members' data, the [data processing agreement](dpa.en.md) is added to these terms.

## 4. Your account

### 4.1 Creating and keeping your account

- An account is an email address that you verify, a name and a password. You can add passkeys and two-factor authentication; two-factor authentication is **mandatory** for Owners and Administrators of an organization, and for any account with the Administer right on a computer.
- Accounts are created and deleted **on the relay's website**, not in the apps. The apps sign in through a system window or through a code that you approve from your phone.
- You give accurate information, in particular your email address: it is how you recover your account and how we warn you about a new device.
- An account is **personal**. Do not share it: to open a computer to someone, add that person with their own account (section 5).

### 4.2 Keeping your account secure

You keep your password, your recovery codes and access to your passkeys secret. You tell us without delay if you think someone else may have got into your account (contact in section 1), and you close open sessions from the account page. Anything done from your account is presumed to have been done by you, unless you had reported an intrusion to us or it results from a fault in our service.

## 5. Computers, rights and remote access

### 5.1 What the service does

The service is used to **take control of computers remotely**: see their screen, hear their sound, and control them with keyboard, mouse and controller. This is powerful. Please read this section carefully.

- **Attaching a computer.** You attach to your account, or to an organization, a computer where KyberNow Station is installed, from that computer (KyberNow Admin or the installer). You only attach computers you are entitled to administer: your own, or those whose owner or employer has allowed you to.
- **Opening a computer to someone.** From the console, you give a person, an organization or a group rights on a computer:
  - **Session**: the computer's whole desktop, with keyboard and mouse;
  - **Lending**: only the applications you have chosen in Admin, during the time slots you have set;
  - **Sharing**: join your shares **without a code and without your approval** each time;
  - **Administer**: manage the computer's access, restart it and update the Station, including through the relay, and open Admin on the network.
- **The relay decides.** When an account has a right on a computer, the relay issues it an access token that the Station accepts without asking for your approval again. You keep control on the Station: the "Accept KyberNow accounts" switch in Admin, the list of access, blocking an account or a device, and an email for each new device.

### 5.2 Your responsibilities for your computers

By attaching a computer and opening it to others, you remain responsible for what you choose:

- **Who you give access to, and which rights.** A person with the Session right can do on the computer anything you could do sitting in front of it: read your files, use accounts already signed in, install software. Only give this right to someone you trust, and remove it when it is no longer needed (an expiry date can do this for you).
- **The people who use the computer.** If other people use the computer (family, colleagues, pupils), you tell them that it can be opened remotely, by whom, and how to see or prevent it.
- **The software and accounts you lend.** The Lending right and sharing let someone else use your applications, games and accounts (a Steam account, for example). You check that the licences and terms of that software and those services allow it; Les Lignes Claires are not answerable for this.
- **Updates.** You keep KyberNow Station and your computers' operating systems up to date.

### 5.3 Your responsibilities on computers opened to you

When someone opens their computer to you:

- you only use the rights you have been given, for the purpose they were given for;
- you do not try to obtain others, nor to get round Lending, time slots, blocks or the Station's safeguards;
- you respect the privacy of the people whose screen, files or conversations you see;
- you leave when asked to, and when your rights are removed.

Accessing or remaining in a computer without authorisation, or altering its data, is a criminal offence (articles 323-1 et seq. of the French Criminal Code).

### 5.4 An organization's computers

An organization that attaches its staff's computers or opens them to its members acts under its own responsibility as employer or controller: informing the people concerned, consulting staff representatives where the law requires, and not using the log to monitor work beyond what the law allows. It is responsible for the data of its members that it entrusts to us, and we process it on its behalf under the [data processing agreement](dpa.en.md).

## 6. Organizations

- An **organization** brings together members with a role: **Owner**, **Administrator** or **Member**, groups and computers. Owners and Administrators add and remove members, grant rights and see the organization's log (who opened what, when, from where, and every change of rights).
- The person who creates the organization is its first Owner. An organization always has at least one Owner: the last one can neither leave nor delete their account without appointing another Owner or deleting the organization.
- When a member leaves the organization, they lose the rights they held through it. Their account remains theirs.
- `[TO DECIDE: what happens to an organization whose Owners no longer respond (recovery procedure, proof required).]`

## 7. Acceptable use

You do not use the service to:

- access a computer without the permission of the person entitled to give it, or monitor someone without their knowledge;
- commit or facilitate an offence, infringe the rights of others (privacy, copyright, trade secrets), harass or deceive anyone;
- send unsolicited invitations or additions in bulk, or use the service's emails to reach people who do not know you;
- attack, probe or overload the relay, its accounts or other users, or get round its limits (number of attempts, number of codes, flows per client);
- resell or offer the public relay as your own service, or use it as a relay for purposes other than KyberNow;
- create accounts in bulk, by automated means, or in someone else's name.

For heavy or professional use beyond what the current offer allows, you can also **run your own relay**: the software is free.

## 8. What we do, and our limits

### 8.1 The service

- We do our best to keep the relay and the console working, secure and up to date. KyberNow is in **beta**: features change, some have not yet been tried everywhere, and interruptions may happen (maintenance, failure, update).
- The public relay is run **in France**, on Les Lignes Claires' hardware. `[TO CHECK: actual hosting at launch, and where backups are kept.]` We guarantee neither a stated availability nor a throughput. `[TO DECIDE: availability commitment for the paid offer.]`
- In the KyberNow Link apps, picture, sound and your input are **end-to-end encrypted** between the app and the computer: the relay cannot read them. In a browser, the relay decrypts the connection to pass it on, and keeps nothing of it ([privacy](confidentialite.en.md)).
- We may change the service: add, change or remove a feature. If a change takes away an important feature from you, we tell you by email in advance.

### 8.2 Liability

- We are liable for damage caused by a breach of these terms on our part, within the limits of the law.
- We are not liable for: what the people **you** gave rights to do; the use you make of computers opened to you; losses related to a computer, network, router or software that is not ours; an interruption due to force majeure; relays run by others.
- `[TO DECIDE with the lawyer: liability cap for professionals; for the free offer.]`
- Nothing in these terms limits our liability for gross negligence or wilful misconduct, for personal injury, nor the rights the law gives you as a consumer, in particular the conformity guarantee for digital services.

> **For the lawyer.** (1) Are limitation clauses valid against a consumer for a free service (article R. 212-1 of the French Consumer Code)? (2) Does the digital content and services regime (articles L. 224-25-1 et seq. of the Consumer Code) apply now, given that the service is free but supplied in exchange for personal data? (3) Is separate wording needed for professionals (organizations)?

## 9. Free today, paid offer to come

- **Today, the service is free.** You are not asked for any means of payment.
- A **paid offer** will come later (for example for organizations or heavier use). When it arrives:
  - we tell you by email **at least `[TO DECIDE: 30]` days** in advance, with its prices and what it includes;
  - **nothing becomes paid without your express agreement**: you are never charged because you keep using the service;
  - if a feature you use for free becomes paid, you can keep it by subscribing, give it up, or delete your account and take your data with you;
  - the paid offer will have its own terms of sale (price, duration, renewal, withdrawal, cancellation, refunds), which you will accept separately.
- A subscription sold in an iOS app goes through Apple's in-app purchase and also follows Apple's rules; an offer sold directly to organizations is settled with us.

> **For the lawyer.** The website currently says "KyberNow is a community, non-commercial project". This sentence will have to change before the paid offer. Also check whether a consumer mediator (articles L. 612-1 et seq.) is required for the free service or only for the paid offer.

## 10. Suspension

We may **suspend** an account, an organization or a computer's access to the relay, in whole or in part:

- in the event of a serious breach of these terms (section 7 in particular);
- to protect the service, other users or a computer, for example if an account appears to be compromised or is used in an attack;
- at the request of an authority, where the law requires us to.

We tell you by email, with the reason, before the suspension where possible, otherwise immediately afterwards, unless the law or an investigation prevents us. You can reply to contest it; we restore the account if the reason disappears. A suspension to protect a compromised account is lifted as soon as you have regained control of it.

## 11. Ending the service and deleting the account

### 11.1 By you

You can **delete your account at any time**, free of charge, from the account page ("Delete account"). Before that, you can **export your data** from the console.

- Your sessions and devices are signed out immediately, and your computers detached: they can no longer be reached by an account, but remain usable on the network, by pairing and by invitation.
- Rights that other accounts had on your computers lapse with them; those you had on other people's computers lapse too.
- If you are the last Owner of an organization, you first appoint another Owner or delete the organization.
- Your data is erased **within 30 days**, backups included, except what the law requires us to keep longer ([privacy](confidentialite.en.md)).

### 11.2 By us

We may close an account:

- after a suspension left unanswered, or in the event of a serious or repeated breach, with `[TO DECIDE: 15]` days' notice by email, except in an emergency;
- `[TO DECIDE: account never verified, erased after 7 days? account inactive for 3 years, erased after two warning emails?]`;
- if we stop the public accounts service: we tell you at least `[TO DECIDE: 3]` months in advance, and you can export your data and move to your own relay.

## 12. Intellectual property

KyberNow's code is free software released under the AGPL-3.0 licence; its source is on GitHub. The KyberNow name, symbol and visual identity are not covered by this licence: you do not use them to suggest that a service or a relay comes from Les Lignes Claires. KyberNow uses Kyber, free software released under the AGPL-3.0 licence, and is neither published nor endorsed by the Kyber team.

What you put into the service (names of computers, organizations, groups) remains yours; you only allow us to keep and display it to run the service.

## 13. Changes to these terms

We may change these terms, for example for the paid offer, a new feature or a new law. For a significant change, we tell you by email and in the console **at least 30 days** before it takes effect, saying what changes. If you do not accept it, you can delete your account before that date. Minor corrections (a typo, an address) apply immediately. Previous versions remain available. `[TO DECIDE: where; keep the date and version accepted by each account.]`

## 14. Governing law and disputes

- These terms are governed by **French law**. If you are a consumer living in another country of the European Union, you keep the protection your country's law gives you.
- If there is a disagreement, write to us first (section 1): we will look for an amicable solution.
- As a consumer, you can also use a consumer mediator free of charge: `[TO COMPLETE: name and address of the mediator]`. You can bring the matter before the competent court under the ordinary rules.
- As a professional, disputes go to the competent court of `[TO DECIDE: Montpellier]`.
- These terms exist in French and English; if they differ, the French version prevails.
