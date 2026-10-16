// SIM-099 regression: an automatic module task writes through its
// virtual-interface formal with blocking and nonblocking whole and selected
// assignments. The written storage belongs to the bound interface instance
// (IEEE 1800-2009 25.9), so a nonblocking write outlives the task's
// activation like any write to a module variable (10.4.2).
`timescale 1ns / 1ns
interface bus_if;
  logic [7:0] data;
  logic [3:0] flags;
endinterface

module tb;
  bus_if a ();
  bus_if b ();

  task automatic drive(virtual bus_if v, logic [7:0] value);
    v.data <= value;
    v.flags[1] <= 1'b1;
    v.flags[3:2] = 2'b10;
    #1 v.data[0] <= 1'b0;
  endtask

  initial begin
    a.flags = 4'b0000;
    b.flags = 4'b0001;
    drive(a, 8'h5d);
    $display("[%0t] a.data=%h a.flags=%b", $time, a.data, a.flags);
    drive(b, 8'h81);
    #1 $display("[%0t] a.data=%h a.flags=%b b.data=%h b.flags=%b", $time, a.data, a.flags,
                b.data, b.flags);
    $finish;
  end
endmodule
