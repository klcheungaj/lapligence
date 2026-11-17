// SV 10.6.2: a concatenation of array elements and variables is a force
// target. Each element is found when the statement executes; an element
// whose index is out of range is left out (SV 7.4.6), and the other parts of
// the concatenation are still forced.
module tb;
  logic [7:0] mem[4];
  logic [7:0] x;
  int i = 1;
  int j = 9;
  initial begin
    foreach (mem[k]) mem[k] = 8'h10 + k;
    x = 8'h00;
    force {mem[i], x} = 16'hab_cd;
    #1 $display("c1 %h %h %h", mem[1], x, mem[2]);
    mem[1] = 8'h55;
    x = 8'h66;
    #1 $display("c2 %h %h", mem[1], x);
    release {mem[i], x};
    #1 $display("c3 %h %h", mem[1], x);
    force {x, mem[j]} = 16'h12_34;
    #1 $display("c4 %h %h %h %h %h", x, mem[0], mem[1], mem[2], mem[3]);
    x = 8'h77;
    #1 $display("c5 %h", x);
    release {x, mem[j]};
    x = 8'h88;
    #1 $display("c6 %h", x);
    $display("done");
    $finish;
  end
endmodule
