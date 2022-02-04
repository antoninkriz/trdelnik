use trdelnik::*;
use fehler::throws;
use program_client::owner_checks_insecure_instruction;
use std::mem;
use spl_token::state::{Account as TokenAccount, AccountState};
use anchor_lang::solana_program::program_option::COption;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::token::TokenAccount as AnchorTokenAccount;

#[trdelnik_test]
async fn test_insecure() {
    // initialize test fixture
    let reader = Reader::with_root("../../../");
    let mut fixture = Fixture {
        client: Client::new(system_keypair(0)),
        program: program_keypair(2),
        program_data: reader.program_data("owner_checks_insecure").await?,
        _authority: system_keypair(3),
        token_account: keypair(4),
        attacker: keypair(5),
    };
    // deploy a tested program
    fixture.deploy().await?;
    // create a token account belonging to the `authority`
    fixture.create_mock_token_account().await?;

    let acc = fixture.client.account_data::<AnchorTokenAccount>(fixture.token_account.pubkey()).await?;
    println!("Token account {:?} succesfully created\n\tamount: {:?}\n\towner: {:?}", 
        fixture.token_account.pubkey(), 
        acc.amount, 
        acc.owner
    );
    // call an intstruction
    owner_checks_insecure_instruction::log_message(
        &fixture.client,
        fixture.token_account.pubkey(),
        fixture.attacker.pubkey(),
        [fixture.attacker]
    ).await?.print();

    println!("{:?}", program_keypair(3).pubkey());
}

struct Fixture {
    client: Client,
    program: Keypair,
    program_data: Vec<u8>,
    _authority: Keypair,
    token_account: Keypair,
    attacker: Keypair,
}

impl Fixture {
    #[throws]
    async fn deploy(&mut self) {
        self.client.airdrop(self.client.payer().pubkey(), 5_000_000_000).await?;
        self.client.deploy(
            self.program.clone(),
            mem::take(&mut self.program_data)
        ).await?;
    }
    #[throws]
    async fn create_mock_token_account(&self) {
        let token_account = 
            TokenAccount {
                mint: Pubkey::default(),
                owner: self.attacker.pubkey(),
                amount: u64::MAX,
                delegate: COption::None,
                state: AccountState::Initialized,
                is_native: COption::None,
                delegated_amount: 0,
                close_authority: COption::None,
            };

        let mut buf = [0; TokenAccount::LEN];
        token_account.pack_into_slice(&mut buf);
        
        self.client.create_account_with_data(
            &self.token_account,
            buf.to_vec()
        ).await?;
    }
}