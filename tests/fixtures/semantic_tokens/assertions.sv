// Semantic-token requirements ST-60 and ST-67 (docs/semantic_tokens.md):
// macro names of every body form are plain keywords, and assertion
// statement words are control flow.
`define ABC 012
`define ABD 0123
`define EMPTY
`define FN(v) (v)
`ifdef ABC
`endif
module asrt_top(input logic clk, input logic a, input logic b);
  int x;
  property p_ok;
    @(posedge clk) a |-> b;
  endproperty
  sequence s_ab;
    a ##1 b;
  endsequence
  a_p: assert property (p_ok) else $error("p");
  m_p: assume property (p_ok);
  c_p: cover property (p_ok);
  c_s: cover sequence (s_ab);
  r_p: restrict property (p_ok);
  initial begin
    x = `ABC + `ABD + `FN(x) `EMPTY;
    assert (x == 1) else $error("e");
    assume (x != 2);
    cover (x == 3) $display("c");
    assert #0 (x >= 0);
    assert final (x >= 0);
    expect (@(posedge clk) a ##1 b) else $error("x");
  end
endmodule
