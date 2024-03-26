use amqprs::{
    callbacks::{DefaultChannelCallback, DefaultConnectionCallback},
    channel::BasicPublishArguments,
    BasicProperties,
};
use async_trait::async_trait;
use log::{debug, error, info};
use serde::Deserialize;
use transmitter_common::{
    data::{KeeperMsg, KeeperMsgImpl, KeeperSignature, OperationData, SignedOperation},
    rabbitmq_client::{RabbitmqBindingConfig, RabbitmqClient, RabbitmqConnectConfig},
};

use super::PublisherError;

#[derive(Deserialize)]
pub(super) struct RabbitmqConfig {
    #[serde(flatten)]
    connect: RabbitmqConnectConfig,
    #[serde(flatten)]
    binding: RabbitmqBindingConfig,
}

pub(super) struct RabbitmqPublisher {
    config: RabbitmqConfig,
}

impl RabbitmqPublisher {
    pub(super) fn new(config: RabbitmqConfig) -> RabbitmqPublisher {
        RabbitmqPublisher { config }
    }

    pub(super) async fn publish_operation_data(
        &self,
        operation_data: OperationData,
        signatures: Vec<KeeperSignature>,
    ) -> Result<(), PublisherError> {
        let connection = self.connect(&self.config.connect, DefaultConnectionCallback).await?;
        let channel = self.open_channel(&connection, DefaultChannelCallback).await?;

        info!(
            "Rabbitmq messaging arguments are: exchange: {}, routing_key: {}",
            self.config.binding.exchange, self.config.binding.routing_key
        );

        let msg = KeeperMsg::V1(KeeperMsgImpl::SignedOperationData(SignedOperation {
            operation_data,
            signatures,
        }));

        let json_data = serde_json::to_vec(&msg).expect("Expected operation be serialized well");

        let args = BasicPublishArguments::from(&self.config.binding);
        if let Err(err) = channel.basic_publish(BasicProperties::default(), json_data, args).await {
            error!("Failed to publish operation_data message, error: {}", err);
            return Err(PublisherError::from(err));
        }
        debug!("operation_data sent: {}", serde_json::to_string(&msg).unwrap());
        Ok(())
    }
}

#[async_trait]
impl RabbitmqClient for RabbitmqPublisher {
    type Error = PublisherError;
}