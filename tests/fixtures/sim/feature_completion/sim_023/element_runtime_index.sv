// SV 10.6.2, 6.4, 7.4.6: an element of a static unpacked array variable is a
// singular variable, so it is a force and release target. The index is
// evaluated when the force or release statement executes; the force stays on
// that element when the index variable changes, and a release of an element
// that is not forced does nothing (industry-standard behaviour).
module tb;
  int mem[4];
  int i = 1;
  initial begin
    foreach (mem[k]) mem[k] = k * 10;
    force mem[i] = 99;
    #1 $display("t1 forced mem[1]: %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    i = 2;
    #1 $display("t2 after i=2: %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    mem[1] = 5; mem[2] = 6;
    #1 $display("t3 after writes 5,6: %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    release mem[i];
    #1 $display("t4 after release mem[i] i=2: %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    mem[1] = 7;
    #1 $display("t5 after write mem[1]=7: %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    i = 1;
    release mem[i];
    mem[1] = 8;
    #1 $display("t6 after release mem[1], write 8: %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    i = 7;
    force mem[i] = 77;
    #1 $display("t7 index out of range: %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    release mem[i];
    $display("done");
    $finish;
  end
endmodule
