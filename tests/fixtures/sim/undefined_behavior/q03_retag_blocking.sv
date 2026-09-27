module tb;
  typedef union tagged packed {
    void V;
    logic [7:0] A;
    logic [7:0] B;
    logic [3:0] C;
  } item_t;
  item_t same_member, other_member, void_member, short_member;
  logic [7:0] seed;
  initial begin
    seed = 8'h11;
    same_member = tagged A(8'h01);
    other_member = tagged A(8'h02);
    void_member = tagged A(8'h03);
    short_member = tagged A(8'h04);
    #1;
    seed = seed + 8'h20;
    same_member.A <= seed;
    other_member.A <= seed + 1;
    void_member.A <= seed + 2;
    short_member.A <= seed + 3;
    same_member = tagged A(8'h51);
    other_member = tagged B(8'h52);
    void_member = tagged V;
    short_member = tagged C(4'h3);
    #1;
    $display("Q03.retag_blocking same=%b other=%b void=%b short=%b", same_member, other_member, void_member, short_member);
  end
endmodule
