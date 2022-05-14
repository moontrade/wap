<script>
	// import "carbon-components-svelte/css/white.css";
	import "carbon-components-svelte/css/g100.css";
	import { Content, Grid } from "carbon-components-svelte";
	import AppHeader from "$lib/AppHeader.svelte";
  </script>
  
  <AppHeader />
  
  <Content style="padding-top: 20">
	<Grid fullWidth>
	  <slot />
	</Grid>
  </Content>