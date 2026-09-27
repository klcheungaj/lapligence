module tb;
  typedef union tagged packed { logic [7:0] A; logic [7:0] B; } item_t;
  item_t whole, member;
  logic [7:0] seed;
  initial begin
    whole = tagged A(8'h11);
    member = tagged A(8'h11);
    seed = 8'h20;
    #1;
    seed = seed + 8'h13;
    whole <= tagged A(seed);
    member.A <= seed;
    whole = tagged B(8'h55);
    member = tagged B(8'h55);
    #1;
    $display("Q03.whole_vs_member whole=%b member=%b", whole, member);
  end
endmodule
