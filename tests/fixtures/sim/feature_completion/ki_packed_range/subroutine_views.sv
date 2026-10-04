// Element ranges of formals, automatic locals and module `ref` ports
// (IEEE 1800-2009 7.4.5, 13.5.2, 23.3.3.3).
module leaf(ref logic [3:0][7:0] value);
  integer i;
  initial begin
    #1;
    value[3:2] = 16'hbbaa;
    i = 0;
    value[i +: 2] = 16'hddcc;
    value[1][3:0] = 4'h0;
    $display("leaf %h %h %h", value, value[2:1], value[i]);
  end
endmodule
module middle(ref logic [4:0][7:0] value);
  leaf child(value[3:0]);
endmodule
module tb;
  logic [5:0][7:0] w [0:1];
  logic [3:0][7:0] r;
  middle m(w[1][4:0]);
  function automatic logic [15:0] pick(input logic [3:0][7:0] v, input int k);
    logic [3:0][7:0] local_copy;
    local_copy = v;
    local_copy[0 +: 2] = local_copy[3:2];
    return local_copy[k -: 2];
  endfunction
  task automatic poke(ref logic [3:0][7:0] v, input int k);
    v[k +: 2] = 16'h0102;
    v[3] = 8'hee;
  endtask
  initial begin
    w[1] = 48'h665544332211;
    #2 $display("w %h", w[1]);
    r = 32'h44332211;
    $display("pick %h %h", pick(r, 3), pick(r, 1));
    poke(r, 1);
    $display("poke %h", r);
    $finish(0);
  end
endmodule
