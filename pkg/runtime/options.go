package runtime

import "time"

type options struct {
	timeout time.Duration
}

// Option configures a runtime client.
type Option func(*options)

// WithTimeout sets the per-call timeout. Defaults to 10s.
func WithTimeout(d time.Duration) Option {
	return func(o *options) { o.timeout = d }
}

func defaultOptions() *options {
	return &options{
		timeout: 10 * time.Second,
	}
}
