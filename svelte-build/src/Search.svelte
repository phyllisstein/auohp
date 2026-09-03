<script>
  import { runSearch, playerUrl, formatTimestamp } from "./query.js";

  let hits = $state([]);
  let error = $state(null);

  // Port of the React useLazyQuery: fire on input, network-only, keep last result.
  let seq = 0;
  async function onInput(e) {
    const term = e.currentTarget.value;
    const mine = ++seq;
    try {
      const data = await runSearch(term);
      if (mine === seq) { hits = data?.search?.interviews ?? []; error = null; }
    } catch (err) {
      if (mine === seq) { error = err; }
    }
  }

  function onResultClick(startTime, interviewNumber) {
    window.location.href = playerUrl(startTime, interviewNumber);
  }
</script>

<div class="search-container">
  <input class="search-input" type="search" oninput={onInput} />

  <!-- React version portals this to document.body as position:fixed.
       Light-DOM Svelte: a plain absolutely-positioned child covers the
       same visual case and keeps the host page able to style it. -->
  <div class="results-container">
    <ul class="search-results">
      {#each hits as hit (`${hit.statement.startTime}-${hit.statement.uid}`)}
        <li class="search-result"
            onclick={() => onResultClick(hit.statement.startTime, hit.interview.number)}>
          <div class="result-match">{hit.statement.text}</div>
          <div class="result-source">{hit.statement.person?.name}</div>
          <div class="result-timestamp">{formatTimestamp(hit.statement.startTime)}</div>
        </li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .search-container { position: relative; font-family: system-ui, sans-serif; }

  .search-input {
    padding: 0.5em;
    border: none;
    border-bottom: 1px solid black;
    font-size: 1.5em;
  }

  .results-container { position: absolute; z-index: 1; width: 100%; }

  .search-results {
    position: absolute;
    z-index: 1;
    overflow: hidden;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: min-content;
    max-height: 400px;
    margin: 0;
    padding: 0;
    list-style: none;
    background-color: white;
  }

  .search-result {
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: auto;
    margin: 0;
    padding: 1rem 0;
  }

  .result-match { grid-column: 1 / 3; grid-row: 1 / 1; padding: 0.5em; font-size: 1em; }
  .result-source { grid-column: 1/2; grid-row: 2/3; padding: 0.5em; font-size: 0.8em; font-weight: 600; }
  .result-timestamp { grid-column: 2/3; grid-row: 2/3; padding: 0.5em; font-size: 0.8em; font-weight: 600; color: grey; }
</style>
