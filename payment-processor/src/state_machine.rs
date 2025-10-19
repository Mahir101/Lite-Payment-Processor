use anyhow::Result;
use shared::{
    PaymentError, Transaction, TransactionEventType, TransactionState,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct TransactionStateMachine;

impl TransactionStateMachine {
    pub fn new() -> Self {
        Self
    }

    pub async fn transition_to_committed(
        &self,
        db: &crate::database::DatabaseService,
        id: Uuid,
    ) -> Result<Transaction, PaymentError> {
        let transaction = db.get_transaction(id).await?
            .ok_or_else(|| PaymentError::TransactionNotFound(id))?;

        if !self.can_transition_to(&transaction.state, &TransactionState::Committed) {
            return Err(PaymentError::InvalidStateTransition(
                transaction.state.clone(),
                TransactionState::Committed,
            ));
        }

        let updated_transaction = db.update_transaction_state(id, TransactionState::Committed).await?;

        // Emit state change event
        db.emit_event(
            &updated_transaction,
            TransactionEventType::StateChanged {
                from: transaction.state,
                to: TransactionState::Committed,
            },
        ).await?;

        Ok(updated_transaction)
    }

    pub async fn transition_to_failed(
        &self,
        db: &crate::database::DatabaseService,
        id: Uuid,
        reason: String,
    ) -> Result<Transaction, PaymentError> {
        let transaction = db.get_transaction(id).await?
            .ok_or_else(|| PaymentError::TransactionNotFound(id))?;

        if !self.can_transition_to(&transaction.state, &TransactionState::Failed) {
            return Err(PaymentError::InvalidStateTransition(
                transaction.state.clone(),
                TransactionState::Failed,
            ));
        }

        let updated_transaction = db.update_transaction_state(id, TransactionState::Failed).await?;

        // Emit failure event
        db.emit_event(
            &updated_transaction,
            TransactionEventType::Failed { reason },
        ).await?;

        Ok(updated_transaction)
    }

    pub async fn transition_to_cancelled(
        &self,
        db: &crate::database::DatabaseService,
        id: Uuid,
    ) -> Result<Transaction, PaymentError> {
        let transaction = db.get_transaction(id).await?
            .ok_or_else(|| PaymentError::TransactionNotFound(id))?;

        if !self.can_transition_to(&transaction.state, &TransactionState::Cancelled) {
            return Err(PaymentError::InvalidStateTransition(
                transaction.state.clone(),
                TransactionState::Cancelled,
            ));
        }

        let updated_transaction = db.update_transaction_state(id, TransactionState::Cancelled).await?;

        // Emit state change event
        db.emit_event(
            &updated_transaction,
            TransactionEventType::StateChanged {
                from: transaction.state,
                to: TransactionState::Cancelled,
            },
        ).await?;

        Ok(updated_transaction)
    }

    fn can_transition_to(&self, from: &TransactionState, to: &TransactionState) -> bool {
        match (from, to) {
            // From PENDING, can go to COMMITTED, FAILED, or CANCELLED
            (TransactionState::Pending, TransactionState::Committed) => true,
            (TransactionState::Pending, TransactionState::Failed) => true,
            (TransactionState::Pending, TransactionState::Cancelled) => true,
            
            // Terminal states cannot transition
            (TransactionState::Committed, _) => false,
            (TransactionState::Failed, _) => false,
            (TransactionState::Cancelled, _) => false,
            
            // Same state is not a transition
            (from, to) if from == to => false,
            
            // All other transitions are invalid
            _ => false,
        }
    }
}



