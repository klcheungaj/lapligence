// Gate item G1: elements of unpacked array members read and written through
// virtual-interface handles (IEEE 1800-2009 25.9), in class code and in
// module code, with blocking and nonblocking assignments, packed selects of
// an element, and a handle rebound between two instances.
`timescale 1ns/1ns
interface bus_if;
  logic [7:0] mem [0:3];
  logic [7:0] grid [0:1][0:2];
  bit [3:0] flags [2];
endinterface

class driver;
  virtual bus_if v;
  task run(virtual bus_if target, int i);
    v = target;
    v.mem[i] = 8'h50 + i;
    v.mem[i + 1] <= 8'h60 + i;
    v.grid[1][i] = 8'ha0 + i;
    v.flags[1] = 4'hc;
    $display("[%0t] i=%0d mem[i]=%h mem[i+1]=%h grid=%h flags=%h low=%h", $time, i,
             v.mem[i], v.mem[i + 1], v.grid[1][i], v.flags[1], v.mem[i][3:0]);
    #1 $display("[%0t] i=%0d after the nonblocking write mem[i+1]=%h", $time, i, v.mem[i + 1]);
  endtask
endclass

module tb;
  bus_if a();
  bus_if b();
  virtual bus_if mv;
  virtual bus_if nv = b;
  // At 2, after the blocking write of grid[0][1] = 33 in the same step.
  initial #2 nv.grid[0][1][0 +: 2] <= 2'b00;
  initial begin
    driver d = new;
    d.run(a, 0);
    d.run(b, 2);
    mv = a;
    mv.mem[3] = mv.mem[0] + mv.mem[1];
    mv.mem[2] = 8'h0f;
    mv.mem[2][7:4] = 4'h9;
    mv.mem[2][1] = 1'b0;
    $display("[%0t] bits %b %b %h %h", $time, mv.mem[2][2], mv.mem[2][1], mv.mem[2][7:4],
             mv.mem[2][2 +: 3]);
    mv = b;
    mv.grid[0][1] = 8'h33;
    #1;
    $display("[%0t] a %h %h %h %h grid %h", $time, a.mem[0], a.mem[1], a.mem[2], a.mem[3],
             a.grid[1][0]);
    $display("[%0t] b %h %h %h %h grid %h %h flags %h", $time, b.mem[0], b.mem[1], b.mem[2],
             b.mem[3], b.grid[1][2], b.grid[0][1], b.flags[1]);
    $finish;
  end
endmodule
