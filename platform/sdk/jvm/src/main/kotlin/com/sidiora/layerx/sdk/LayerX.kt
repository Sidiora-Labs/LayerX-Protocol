@file:JvmName("LayerXKotlin")

package com.sidiora.layerx.sdk

import java.math.BigInteger
import java.util.concurrent.CompletionStage
import kotlin.reflect.KClass

public fun protocolAmount(value: BigInteger): ProtocolAmount = ProtocolAmount.of(value)
public fun protocolAmount(value: String): ProtocolAmount = ProtocolAmount.parse(value)
public fun idempotencyKey(value: String): IdempotencyKey = IdempotencyKey(value)
public fun streamCursor(value: String): ResumableStream.Cursor = ResumableStream.Cursor(value)

public fun <T : Any> ProductionClient.agent(
    operation: String,
    request: Any,
    responseType: KClass<T>,
    options: ProductionClient.Options = ProductionClient.Options.none(),
): CompletionStage<T> = agent(operation, request, responseType.java, options)

public fun <T : Any> ProductionClient.human(
    operation: String,
    request: Any,
    responseType: KClass<T>,
    options: ProductionClient.Options = ProductionClient.Options.none(),
): CompletionStage<T> = human(operation, request, responseType.java, options)

public fun IdempotencyKey.asOptions(
    pathParameters: Map<String, String> = emptyMap(),
): ProductionClient.Options = ProductionClient.Options(this, pathParameters)
