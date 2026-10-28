// Synthetic data: random base64 under valid protect pragmas, no real key.
module tb;
  logic a, b;
  secret_cut u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule

module secret_cut(input logic a, output logic b);
`pragma protect begin_protected
`pragma protect encrypt_agent="synthetic", encrypt_agent_info="adv_030 test vector"
`pragma protect key_keyowner="none", key_keyname="none", key_method="rsa"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=129)
`pragma protect key_block
JXHwzcsL+wkVxyeha4jKE0pUELoJMn/myE9+b4XZIzaT6ZomhU4YdQtKqU99iTm5
BBeWORVw4okHdogM1Km3+OJDVL1PaSyQhTBrG2IfVaNNaZuVAJm3oWbxUctKxm5L
xCrVkXGYpNyG7GJisoLKnCKJc4A/ODpLeM37AuOCFOkV
`pragma protect data_method="aes128-cbc"
`pragma protect encoding=(enctype="base64", line_length=64, bytes=150)
`pragma protect data_block
PMF+T6FdPJH9jQNiD2dWxoJVy29lHJMd6+Uyaxmy4Wl3UU4SXaB6jue0cWBF6oXV
MdImDZtSIIPt5maetqHjiX65EIqe8kfm1SZvaY7Ag2mnBXlNBlav214oaljrYUBU
n7H/jkSN17DAcYFoUsNfpjQ7BFtkW/FczvtU1UyqLhONSiJPpg/efAqKXwHD/FYU
XC/7WRVu
