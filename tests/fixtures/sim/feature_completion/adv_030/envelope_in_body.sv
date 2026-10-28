// Synthetic data: random base64 under valid protect pragmas, no real key.
module secret_body(input logic a, output logic b);
`pragma protect begin_protected
`pragma protect encrypt_agent="synthetic", encrypt_agent_info="adv_030 test vector"
`pragma protect key_keyowner="none", key_keyname="none", key_method="rsa"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=129)
`pragma protect key_block
bV52vwUoMAAeGs1w25gMJqQKjgkCvLCqeOoX5szfa1iIkFpTvMl5GEWQ6ce5LPIj
4nPX6cZrElisbUd7ViTqJqddNAFpo/ogJtyJKb5TL6JVQ8clWWj8+BywH4cM73pf
p2PwHVG4Pr6a2YPo0JNLLEmanE0aSD+KuGroxv+JZS5R
`pragma protect data_method="aes128-cbc"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=96)
`pragma protect data_block
ARrTirptc9RRwZc9x9nrhJSVLQQhAu4LWmWFXYeaOTWg3nghvCb4ZFAimEC0/vaL
cBAH8O64KmOiudh3Wt2RREaSLrSaU+s/a4cf54FCuuPDOwe8R6wIH3OzDCHayz2M
`pragma protect end_protected
endmodule

module tb;
  logic a, b;
  secret_body u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule
