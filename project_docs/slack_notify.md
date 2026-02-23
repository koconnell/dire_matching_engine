# Slack notifications for Cloud Build

When a build **succeeds**, Cloud Build can post a message to a Slack channel. The notification step runs only after tests, build, and (if configured) deploy all succeed.

---

## Create or find your webhook URL

You need a **Slack Incoming Webhook** URL. Slack doesn’t show a list of “my webhooks” in the UI; you create one per channel and copy the URL once.

### Option A: Create a new webhook (recommended)

1. **Open the Incoming Webhooks page**
   - Go to: **https://api.slack.com/messaging/webhooks**
   - Or: in Slack click your **workspace name** (top left) → **Settings & administration** → **Manage apps** → search **“Incoming Webhooks”** → open it.

2. **Add the app to your workspace**
   - Click **“Add to Slack”** (or “Add to [your workspace]”).
   - If you see “Incoming Webhooks is already installed”, go to step 3.

3. **Pick the channel**
   - Choose the channel where build messages should go (e.g. `#builds`, `#engineering`, or `#deployments`).
   - Click **“Allow”** / **“Add Incoming Webhooks integration”**.

4. **Copy the Webhook URL**
   - On the setup page you’ll see **“Webhook URL for your workspace”**.
   - It looks like:  
     `https://hooks.slack.com/services/T01234ABCD/B05XYZ123/abcdefGHIjklMNOpqr`
   - Click **“Copy”** or select and copy the full URL. You’ll paste this into your Cloud Build trigger.

5. **Optional:** set a name (e.g. “Cloud Build”) and icon, then **Save**.

### Option B: You already added “Incoming Webhooks” before

- Slack doesn’t list existing webhook URLs in the UI.
- **Easiest:** create a **new** webhook (Option A) for the channel you want; you can have multiple webhooks for different channels.
- **If you have the old URL:** it still works. It’s in the form  
  `https://hooks.slack.com/services/T…/B…/…`  
  Paste that into the trigger.

### Option C: Create via a custom Slack app (same URL type)

1. Go to **https://api.slack.com/apps** → **Create New App** → **From scratch**.
2. Name it (e.g. “Build Notifications”), pick your workspace, create.
3. In the app: **Incoming Webhooks** → turn **On** → **Add New Webhook to Workspace** → choose channel.
4. Copy the **Webhook URL** shown there. Use that in Cloud Build.

## 2. Add the webhook to your trigger

- Open **Cloud Build** → **Triggers** → your trigger → **Edit**.
- Under **Substitution variables**, add:
  - **Variable:** `_SLACK_WEBHOOK_URL`
  - **Value:** your webhook URL (paste the full URL).

Save the trigger. The next successful build will post to that channel.

**Security:** The URL will appear in the trigger config. For production, use Secret Manager (see below) so the URL is not stored in plain text.

## 3. (Optional) Use Secret Manager

To avoid storing the webhook URL in the trigger:

1. **Create a secret** (once):
   ```bash
   echo -n "https://hooks.slack.com/services/YOUR/WEBHOOK/URL" | \
     gcloud secrets create slack-webhook-url --data-file=- --project=YOUR_PROJECT
   ```
2. **Grant Cloud Build access** to the secret:
   - IAM: ensure the Cloud Build service account has **Secret Manager Secret Accessor** on that secret.
3. **In the trigger**, under **Substitution variables**, you cannot reference a secret directly. So either:
   - Keep using the substitution variable (URL in trigger), or
   - Use a **build trigger** that runs a first step to fetch the secret and export it, then the notify step uses it (more involved).

For most teams, using the substitution variable and restricting who can edit the trigger is acceptable.

## What gets posted

On **success**, Slack receives a message like:

- **Build succeeded**
- Repo: dire-matching-engine
- Commit: (short SHA or “local”)
- Region: (e.g. us-central1)
- Link: **View in Cloud Build** (opens the build in GCP Console)

If `_SLACK_WEBHOOK_URL` is not set, the notify step does nothing and the build continues as before.

## Troubleshooting: build passed but no message in Slack

1. **Check the build log for the “notify-slack” step**
   - Open the build in **Cloud Build → History** and expand **Step 4: notify-slack**.
   - If you see: **“Slack webhook not set; skipping notification”**
     - The trigger does not have `_SLACK_WEBHOOK_URL` set, or it’s empty.
     - Edit the trigger → **Substitution variables** → add `_SLACK_WEBHOOK_URL` with your full webhook URL (no spaces, full `https://hooks.slack.com/...`).
   - If you see: **“Slack response HTTP code: 404”** (or 400, 401, etc.)
     - The URL is wrong, or the webhook was removed/revoked. Create a new webhook (see above) and update the trigger.
   - If you see: **“Slack response HTTP code: 200”** and **“Slack notification sent.”**
     - The request succeeded. Check that you’re looking in the **channel you chose when creating the webhook** (each webhook posts only to that channel).

2. **Confirm the trigger has the variable**
   - **Cloud Build → Triggers** → your trigger → **Edit**.
   - Under **Substitution variables** you must have **Variable:** `_SLACK_WEBHOOK_URL` and **Value:** your webhook URL.
   - Save. Re-run the trigger (e.g. push a commit or “Run trigger”) and check the **notify-slack** step log again.

3. **Test the webhook from your machine**
   ```bash
   curl -X POST -H "Content-Type: application/json" \
     -d '{"text":"Test from Cloud Build setup"}' \
     "YOUR_WEBHOOK_URL"
   ```
   - If a “Test from Cloud Build setup” message appears in Slack, the URL is valid and the channel is correct.
   - If you get an error or no message, create a new webhook and use that URL.

4. **Image not allowed**
   - If the **notify-slack** step fails with an image pull or “not allowed” error, your project may restrict container images.
   - In that case we can switch the step to use a different image (e.g. `gcr.io/cloud-builders/gcloud` with `curl`). Ask for that change and we can update the config.

## Failure notifications

This setup only notifies on **success**. To notify on **failure**, you’d add a separate mechanism (e.g. Cloud Functions triggered by Pub/Sub on build failure, or a second trigger that runs on failure and calls Slack). That is not included in this config.
