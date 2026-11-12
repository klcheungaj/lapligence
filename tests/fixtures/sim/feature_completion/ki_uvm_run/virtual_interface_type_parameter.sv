`timescale 1ns/1ns
// A virtual interface type reached first through a class type parameter
// (`holder::T`), then through its own name: both name one interface type.
interface pins;
  int v;
endinterface

class holder #(type T = int);
  T val;
  function void write(T t);
    val = t;
  endfunction
endclass

module tb;
  pins a();
  pins b();
  initial begin
    static holder #(virtual pins) h = new;
    virtual pins direct;
    h.write(a);
    h.val.v = 5;
    direct = b;
    direct.v = 7;
    h.write(direct);
    h.val.v += 1;
    $display("a.v=%0d b.v=%0d same=%0d", a.v, b.v, h.val == b);
    $finish;
  end
endmodule
