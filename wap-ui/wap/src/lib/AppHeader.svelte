<script>
    import {
      SkipToContent,
      Header,
      HeaderNav,
      HeaderNavItem,
      HeaderUtilities,
      HeaderGlobalAction,
    } from "carbon-components-svelte";
    import {
      Notification,
      UserAvatar,
      Apps,
    } from "carbon-icons-svelte";
    import { page } from "$app/stores";
  </script>
  
  <Header company="Moontrade" platformName="WAP" href="/">
    <div slot="skip-to-content">
      <SkipToContent />
    </div>
    <HeaderNav>
      <!-- <HeaderNavItem
        text="Markets"
        href="/repos"
        sveltekit:prefetch
        aria-current={$page.url.pathname === "/markets" ? "page" : undefined}
      /> -->
      <HeaderNavItem
        text="Repositories"
        href="/repos"
        sveltekit:prefetch
        aria-current={$page.url.pathname === "/repos" ? "page" : undefined}
      />
    </HeaderNav>
    <HeaderUtilities>
      <HeaderGlobalAction aria-label="Notifications" icon={Notification} />
      <HeaderGlobalAction aria-label="User Avatar" icon={UserAvatar} />
      <HeaderGlobalAction aria-label="App Switcher" icon={Apps} />
    </HeaderUtilities>
  </Header>