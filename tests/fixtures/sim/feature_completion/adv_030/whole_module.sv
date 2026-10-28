// Synthetic data: random base64 under valid protect pragmas, no real key.
module tb;
  logic a, b;
  secret_whole u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule

`pragma protect begin_protected
`pragma protect encrypt_agent="synthetic", encrypt_agent_info="adv_030 test vector"
`pragma protect key_keyowner="none", key_keyname="none", key_method="rsa"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=129)
`pragma protect key_block
xv81JszWjwHp16KUwf6/V57ld1/qoeBQJmg5AVUrjbTXoJCXsBPMxG0b3NxgcDB0
gf4KxALsOM8kOM3Lb3KTHlyl1oOklpqDobCqJtJJNnwgSC5Vstj7T4uwwlSMtAsB
4fhEN8gyuLFjEi0ayDZ+RQHwLPD74dtX4thGoibuVebF
`pragma protect data_method="aes128-cbc"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=162)
`pragma protect data_block
q3oUIuLAFNKGBLHa3WN4W7n5R616Hc4lsOfkQv9PFtkRyjHdyOxluCiXpSvNu7h6
0JWUFqI3/A1p7mHnQY/oTJELjDIgXUqc8KM/6m1MsBo4LqBM8jvJy/TzaUQaBpzq
r5hXuZnhMPc+MgBrbANJugreZ7PKjcixNA3zU+Bk4VghmAFbkZogu5uGK5CW7Qjc
kmI78ZluGo6MHtm68k2MsCds
`pragma protect end_protected
