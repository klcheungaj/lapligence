module sub;
  reg [3:0] x;
endmodule

module tb;
  reg [7:0] source;
  reg [7:0] whole;
  reg [3:0] high, low;
  integer count;
  real level;
  sub u ();
  initial begin
    source = 8'h12;
    assign whole = source;
    #1 $display("whole=%h", whole);
    source = 8'h34;
    #1 $display("whole=%h", whole);
    deassign whole;
    source = 8'h56;
    #1 $display("kept=%h", whole);
    whole = 8'h99;
    #1 $display("written=%h", whole);
    assign {high, low} = source;
    #1 $display("pair=%h%h", high, low);
    source = 8'h9a;
    #1 $display("pair=%h%h", high, low);
    deassign high;
    deassign low;
    source = 8'h00;
    #1 $display("pair-kept=%h%h", high, low);
    assign count = 41 + 1;
    #1 $display("count=%0d", count);
    assign level = 1.5;
    #1 $display("level=%0.2f", level);
    deassign level;
    assign u.x = 4'h7;
    #1 $display("hier=%h", u.x);
    deassign u.x;
    $finish;
  end
endmodule
