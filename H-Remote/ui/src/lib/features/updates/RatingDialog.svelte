<script lang="ts">
  import Modal from "../../components/Modal.svelte";
  import Button from "../../components/Button.svelte";
  import { commands } from "../../ipc/client";
  import { app } from "../../state/app.svelte";

  async function rateNow(): Promise<void> {
    await app.decideRating("rated");
    const url = app.bootstrap?.feedbackUrl;
    if (url) {
      try {
        await commands.openUrl(url);
      } catch (error) {
        await app.reportError(error);
      }
    }
  }
</script>

<Modal
  open={app.ratingOpen}
  title="Enjoying H-Remote?"
  description="A short note helps prioritise the next features."
  width="sm"
  onclose={() => void app.decideRating("later")}
>
  <p class="hr-small hr-muted">
    Ratings are not collected by the app; the feedback page opens in your browser. You can turn
    this prompt off at any time.
  </p>

  {#snippet footer()}
    <Button variant="ghost" onclick={() => void app.decideRating("never")}>Don't ask again</Button>
    <Button variant="secondary" onclick={() => void app.decideRating("later")}>Later</Button>
    <Button variant="primary" onclick={() => void rateNow()}>Rate now</Button>
  {/snippet}
</Modal>
