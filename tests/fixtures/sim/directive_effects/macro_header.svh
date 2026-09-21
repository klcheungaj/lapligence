// IEEE 1364-2001 section 19.3.1 and IEEE 1800-2009 section 22.5.1:
// guarded macro include.
`ifndef SYN017_HEADER_SVH
`define SYN017_HEADER_SVH
`define SYN017_WIDTH 4
`define SYN017_CAT(a,b) a``b
`define SYN017_STR(value) `"value`"
`endif
