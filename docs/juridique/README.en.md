# KyberNow service legal texts (drafts)

English · [Français](README.md)

Drafts of the legal texts for the KyberNow accounts service (public relay `share.kybernow.app`), written from the specification ([comptes.en.md](../comptes.en.md)), from what the relay does ([relais.en.md](../relais.en.md)) and from the website's current legal notice. **Nothing is in force or published**: everything must be reviewed by a lawyer before accounts go live (batch 6 of the specification). Version 0.1 of 23/11/2026. The French versions prevail.

## The documents

| Document | Purpose | Published? |
|---|---|---|
| [conditions.en.md](conditions.en.md) · [FR](conditions.md) | Terms of use: accounts, public relay, organizations, acceptable use, remote access, suspension, deletion, free service and paid offer, governing law | Yes, on the relay (linked from "Create an account") |
| [confidentialite.en.md](confidentialite.en.md) · [FR](confidentialite.md) | GDPR privacy policy for the service | Yes, on the relay; it replaces the current `/privacy` page |
| [registre.md](registre.md) (French only) | Record of processing activities (article 30) | No: internal document, shown to the CNIL on request |
| [dpa.en.md](dpa.en.md) · [FR](dpa.md) | Data processing agreement (article 28) for customer organizations | Yes, accepted by an Owner when an organization is created |

In the texts, `[TO COMPLETE]` awaits information, `[TO DECIDE]` a decision, `[TO CHECK]` a confirmation; the "For the lawyer" notes raise legal questions and are removed from the published versions.

## Priorities for the lawyer

1. **GDPR roles**: Les Lignes Claires as controller for accounts, processor for organizations' members, rights and log; overlap between the account's security log and the organization's log.
2. **Liability**: limitation clauses against a consumer for a free service; the digital content and services regime (French Consumer Code, articles L. 224-25-1 et seq.); separate wording for professionals.
3. **Remote access**: the split of responsibilities (whoever opens their computer, whoever comes in, the employer organization) and the reference to articles 323-1 et seq. of the French Criminal Code.
4. **Retention**: IP address kept 12 months in the log versus a few days in the technical logs; whether decree no. 2021-1362 (retention of identification data) applies, which could contradict erasure within 30 days.
5. **Impact assessment** (DPIA) before organizations launch, and whether to appoint a data protection officer.
6. **Notification time** to the Controller in the data processing agreement (48 hours proposed).
7. **Minimum age** and accounts held by minors.
8. **Consumer mediator**: required for the free service, or only for the paid offer.

## To complete or decide

**Identity** (taken from the website's legal notice; to confirm at launch, and to change everywhere if a company replaces the sole trader):

- publisher and controller: Matthieu Requenna, sole trader, trading as Les Lignes Claires;
- SIREN 510 178 577, address in Montpellier;
- dedicated contact address for the service and for data (currently contact@leslignesclaires.fr); vulnerability reporting address (`security.txt`).

**Providers**:

- **EU email provider**: name, address, data location, signed processing agreement, retention of its sending logs, no transfer outside the EU on its side;
- **relay hosting**: currently on the publisher's hardware, in France (the `/privacy` page says "at the KyberNow author's home"); to confirm, with the internet provider, and to revisit if the relay moves to a host;
- **backups**: location, frequency, encryption, retention; provider if external.

**Retention**:

- relay technical logs: maximum period in days (currently "a few days", rotated by size);
- inactive device sessions; console session cookie;
- accounts never verified (7 days proposed), inactive accounts (3 years proposed, after warning);
- people added by email who never create an account (30 days proposed);
- an organization's log lines after a member's account is deleted;
- support messages, rights requests, breach record.

**Terms**:

- minimum age (15 proposed);
- notice before the paid offer (30 days proposed), before we close an account (15 days), before the service stops (3 months);
- liability cap; availability commitment for the paid offer;
- recovery of an organization whose Owners cannot be reached;
- competent court for professionals (Montpellier proposed);
- consumer mediator;
- processing agreement accepted online, or signed; on-site audit while the relay is at the publisher's home.

## What these texts ask of the product

To carry into batch 1 and the following ones (no code is changed here):

- keep **the version and date of the terms accepted** by each account, and by each organization for the processing agreement (no field planned in the batch 1 database);
- "terms and privacy" tick box on the sign-up page linking to these texts; previous versions available;
- **data export** for the account and, for an organization, its members, rights and log;
- erasure **within 30 days, backups included**: plan backup rotation accordingly;
- automatic erasure of unverified accounts, people added without an account, and (if decided) inactive accounts with warning emails;
- email to an organization's Owners for a new sub-processor (30 days before) and for a breach (48 hours);
- the relay's `/privacy` page to be replaced by the privacy policy; the website's legal notice to be updated (the relay holds accounts; the sentence "community, non-commercial project" will have to change before the paid offer).
