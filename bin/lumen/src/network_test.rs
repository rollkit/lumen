#[cfg(test)]
mod tests {
    use crate::{RollkitArgs, RollkitNetworkBuilder};
    use clap::Parser;
    
    #[test]
    fn test_network_builder_with_gossip_disabled() {
        // Create a network builder with gossip disabled
        let builder = RollkitNetworkBuilder::new(true);
        assert!(builder.disable_tx_pool_gossip);
    }
    
    #[test]
    fn test_network_builder_with_gossip_enabled() {
        // Create a network builder with gossip enabled (default)
        let builder = RollkitNetworkBuilder::new(false);
        assert!(!builder.disable_tx_pool_gossip);
    }
    
    #[test]
    fn test_rollkit_args_default() {
        let args = RollkitArgs::default();
        assert!(!args.disable_tx_pool_gossip);
        // Note: enable_rollkit defaults to false with #[derive(Default)]
        // but is set to true when parsed from CLI
        assert!(!args.enable_rollkit);
    }
    
    #[test]
    fn test_rollkit_args_parsing() {
        // Test parsing with gossip disabled
        let args = RollkitArgs::parse_from(["lumen", "--disable-tx-pool-gossip"]);
        assert!(args.disable_tx_pool_gossip);
        
        // Test parsing without the flag (default)
        let args = RollkitArgs::parse_from(["lumen"]);
        assert!(!args.disable_tx_pool_gossip);
    }
}