# Code signing: what to buy and where to put it

Unsigned apps trigger Windows SmartScreen ("Windows protected your PC") and macOS Gatekeeper
("can't be opened because Apple cannot check it" or "is damaged"). Signing removes those warnings. The release
workflow turns signing on by itself once these repository secrets exist
(GitHub → sriptcollector/cereal → Settings → Secrets and variables → Actions).

## Windows: Azure Trusted Signing (cheapest that works with CI)

- **Buy:** an Azure subscription with **Trusted Signing**, Basic plan, about $9.99/month.
- **Identity validation:** Microsoft verifies the person or company. For an individual it's done in the
  Azure portal with a government ID. A company needs to have existed for 3+ years for "public trust",
  otherwise sign as an individual.
- **Create:** a Trusted Signing account and a certificate profile (Public Trust), and an app
  registration (service principal) with the role "Trusted Signing Certificate Profile Signer".
- **Secrets to add:** `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`,
  `TRUSTED_SIGNING_ACCOUNT`, `TRUSTED_SIGNING_PROFILE`.
- SmartScreen reputation builds up over the first few hundred downloads even when signed.
  An EV certificate (about $250-400/year from SSL.com, Sectigo or DigiCert, on a hardware token)
  skips that ramp but is harder to use from CI.

## macOS: Apple Developer Program

**Today (no Apple account):** every Mac build is ad-hoc signed with the hardened runtime and
`src-tauri/Entitlements.plist`; each bundled engine program and dylib is signed first with
`src-tauri/Helper.entitlements`. CI fails the release if `codesign --verify --deep --strict` fails on
the app or the app inside the DMG, if any engine file is unsigned, or if an engine doesn't start.
The `mac-check` workflow (Actions → mac-check → tag) installs a published DMG the way a user does
and runs `codesign`, `spctl` and `syspolicy_check` on it.

That is as far as ad-hoc signing goes: a downloaded app that isn't notarized is still blocked by
Gatekeeper. On recent macOS (15 Sequoia, 26) the message often says the app **"is damaged and can't
be opened"** even though the signature is valid. Until there's a Developer ID, each release also has
`Cereal_x.y.z_<arch>.pkg`: an unsigned installer package gets the softer "unidentified developer / Open Anyway"
warning, and the app the macOS Installer places in /Applications isn't quarantined (CI installs it and checks).
With the DMG instead, Mac users run this once after dragging Cereal to Applications:

```bash
xattr -dr com.apple.quarantine /Applications/Cereal.app
```

**With an Apple Developer account** the same workflow signs with your Developer ID, notarizes with
`notarytool`, staples the ticket and checks it with `spctl`. Nothing to change in the code: add the
secrets and tag a release.

### Steps (about 30 minutes, plus Apple's review of 1-2 days)

1. **Enroll** at https://developer.apple.com/programs/enroll/ with your Apple ID (two-factor
   authentication must be on). Choose **Individual** (fastest; your name shows as the developer) or
   **Organization** (needs a D-U-N-S number). Pay $99/year. Wait for the "Welcome" email.
2. **Team ID:** https://developer.apple.com/account → Membership details → *Team ID* (10 characters).
3. **Certificate** (on a Mac):
   - Keychain Access → Certificate Assistant → *Request a Certificate From a Certificate Authority*,
     your email, *Saved to disk*.
   - https://developer.apple.com/account/resources/certificates → **+** → **Developer ID
     Application** → upload that request → download the `.cer` and double-click it.
   - Keychain Access → *My Certificates* → right-click *Developer ID Application: Your Name (TEAMID)*
     → **Export** → `.p12` with a password you choose.
   - `base64 -i Certificates.p12 | pbcopy` (copies it as text).
4. **App-specific password:** https://account.apple.com → Sign-In and Security → **App-Specific
   Passwords** → **+**, name it "toolbay notarize", copy it.
5. **Add the secrets** to each repo that builds a Mac app (cereal, toolbay-desktop,
   toolbay-notes): GitHub → repo → Settings → Secrets and variables → Actions → *New repository secret*.

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | the base64 text from step 3 |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` export password |
| `APPLE_ID` | your Apple ID email |
| `APPLE_TEAM_ID` | the Team ID from step 2 |
| `APPLE_APP_PASSWORD` | the app-specific password from step 4 |
| `APPLE_SIGNING_IDENTITY` | optional: `Developer ID Application: Your Name (TEAMID)` (found by name otherwise) |

6. Tag a release. The "Verify macOS signature" step should show `Authority=Developer ID
   Application…`, `source=Notarized Developer ID` and "The validate action worked!".

## Update signing (already set up)

`TAURI_SIGNING_PRIVATE_KEY` signs the update packages that installed copies download. It's a
free key generated locally (kept at `~/.tauri/cereal-updater.key` on the build PC, never in
the repo). Its public half is in `src-tauri/tauri.conf.json`. Back it up: losing it means existing
installs can't auto-update.
