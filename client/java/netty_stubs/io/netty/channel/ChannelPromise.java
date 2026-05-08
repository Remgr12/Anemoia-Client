package io.netty.channel;
public interface ChannelPromise {
    ChannelPromise setSuccess();
    ChannelPromise setFailure(Throwable cause);
}
