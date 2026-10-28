// Synthetic data: random base64 under valid protect pragmas, no real key.
module tb;
  logic a, b;
  secret_header u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule

module secret_header(input logic a, output logic b);
`pragma protect begin_protected
`pragma protect encrypt_agent="synthetic", encrypt_agent_info="adv_030 test vector"
`pragma protect key_keyowner="none", key_keyname="none", key_method="rsa"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=129)
`pragma protect key_block
HJ+0joRD5gO0kIf3rFUxhHnBXLZShwB1z9XaH+12tBFmtNcap25fcJSn0/EHu37F
IYi9ok+tZzWdCDRbiMB8FfIsdRYeiTsmGJSKJ/Z+PUXrUIWFBEk6pwpwZTJLeKwk
GI0YTj5rNsQsSraMx8lxWcoGvZTcd2ckBVWjeV4TRq+5
`pragma protect data_method="aes128-cbc"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=120)
`pragma protect data_block
5yD/49mw4Z/y0C4+JQDjmZe64hj9O2M3PmdRTIFnvwzoQQfPalXn4r/ofluep0eE
kkezL17R+i11FvAOmVQ6kdlxcfhj/5dCWkJPbFrLdK/c/RIdrOZxh99ox3srmL+y
YS0re95TWa43OPg2WyBsjXvkd7eDxlWP
`pragma protect end_protected
