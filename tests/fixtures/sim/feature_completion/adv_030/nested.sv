// Synthetic data: random base64 under valid protect pragmas, no real key.
module secret_nested(input logic a, output logic b);
`pragma protect begin_protected
`pragma protect encrypt_agent="synthetic", encrypt_agent_info="adv_030 test vector"
`pragma protect key_keyowner="none", key_keyname="none", key_method="rsa"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=129)
`pragma protect key_block
djBy+kvxQQW/TW0alsuksWSdRg05aCAbdVP8NnVC2n31xR1enrkyHLxywxXv/r0X
wQLwgIwujawW16ssoh4lCZd0F6iZf9uKk3hrIRp0RB+2WOz1Wbk5+nkxCB/LPTzG
UwGsYKTlsMc1g0/+v1ykcpIdTji9DQLxL8MAUIU4N2gt
`pragma protect begin_protected
`pragma protect encrypt_agent="synthetic", encrypt_agent_info="adv_030 test vector"
`pragma protect key_keyowner="none", key_keyname="none", key_method="rsa"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=129)
`pragma protect key_block
z9ExYgOenAdKCkJ+fSJX4l+4K2OhTVAAIsGdUP2N/NpE1VOikgOFyNP+tiqWRfto
ZI0jXsjvuBKPpxG8u1vOATz7tjrUdowtDlxLHk5qTtmBYUQlryl4Tfgxrv2LAd2o
jZZBexseLtn9u9kwuv+Xh1pz4uxepq6+VkCeK7xdJCCh
`pragma protect data_method="aes128-cbc"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=60)
`pragma protect data_block
lmI9t2jKl5OISeiF9c3I921xrMYMAbLCkWMThZH+B8WGZYRWUPsNOw8/ZIUsMcQm
0VxZ69gTSvpmsd0v
`pragma protect end_protected
`pragma protect data_method="aes128-cbc"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=90)
`pragma protect data_block
jYAAeBIChp4nF0jbPppwaKye+LIVVkNSlPmvMflblKBZMMGLdwtUNphcizb2YAlz
8s1AURIQz7b8S4l+gAZRmjPq03XpCPbf425ubzbVbdYR9LrtW3ZyMGBn
`pragma protect end_protected
endmodule

module tb;
  logic a, b;
  secret_nested u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule
