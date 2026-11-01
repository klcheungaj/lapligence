// Decision A19-D64: the value of a switch and the name of a class object.
// IEEE 1800-2009 37.31 (L59775-59777) gives every primitive "-> value
// vpi_get_value()"; a switch has no output terminal, and the text does not
// say which terminal gives its value. 37.29 draws "-> name str: vpiName"
// for a class obj without saying what the name is.
// Decision: the value of a switch is the value of its first terminal (as a
// gate's is its output's); a class object's vpiName is the name of its
// class, and it has no vpiFullName.
`timescale 1ns/1ns
module tb;
  wire k1, k2, k3, k4, k5, k6;
  logic en1 = 1'b1, en0 = 1'b0, d1 = 1'b1, d0 = 1'b0;
  assign k1 = d1;
  assign k3 = d1;
  assign k6 = d0;
  tranif1 sw1 (k1, k2, en1);
  tranif1 sw0 (k3, k4, en0);
  tran sw2 (k5, k6);
  class Packet;
    int id;
  endclass
  class Burst extends Packet;
    int len;
  endclass
  Packet p;
  Burst b;
  initial begin
    p = new;
    b = new;
    #1 $d64;
  end
endmodule
