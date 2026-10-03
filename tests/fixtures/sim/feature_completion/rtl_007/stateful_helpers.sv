// SV2009 9.4.2, 13.4, 6.5, 10.3: legal zero-time helpers with visible writes
// or persistent static state in evaluated events and continuous assignments.
// The language does not fix how often an event expression is evaluated, so
// evaluation counts are only checked as lower bounds. Writes made inside a
// function called by a continuous assignment are procedural writes; they may
// share storage with other procedural writers.
module tb;
  logic [3:0] v;
  logic en;
  logic [7:0] x, y_cont, z_cont;
  int evals = 0, hits = 0, cont_calls;
  int stamp = 0;

  function automatic logic [3:0] look(input logic [3:0] value);
    evals++;
    return value;
  endfunction
  function automatic bit gate(input logic e);
    evals++;
    return e;
  endfunction
  // Persistent static state: the result depends on the previous call.
  function logic [3:0] delta(input logic [3:0] value);
    logic [3:0] last;
    delta = value ^ last;
    last = value;
  endfunction
  function logic [7:0] counted(input logic [7:0] value);
    cont_calls = cont_calls + 1;
    return value + 8'd1;
  endfunction
  function automatic logic [7:0] stamped(input logic [7:0] value);
    stamp = 32'(value);
    return value << 1;
  endfunction

  task automatic waiter(input int id);
    logic [3:0] seen;
    @(posedge look(v));
    seen = v;
    $display("task_posedge %0d %b %0t", id, seen, $time);
  endtask

  assign y_cont = counted(x);
  assign z_cont = stamped(x);
  always @(look(v) iff gate(en)) hits++;

  initial begin
    v = 4'bxxx0;
    en = 0;
    x = 8'd3;
    cont_calls = 100;
    fork
      waiter(1);
      begin @(delta(v)) $display("static_state %b %0t", v, $time); end
      begin
        #1 v = 4'b0010;
        #1 v = 4'b0011;
        #1 en = 1;
        #1 v = 4'b0111;
        #1 v = 4'b0111;
      end
    join
    #1 $display("qualified hits=%0d evaluated=%0d", hits, evals > 3);
    $display("continuous %0d %0d calls=%0d stamp=%0d", y_cont, z_cont, cont_calls > 100, stamp);
    x = 8'd20;
    #1 $display("continuous %0d %0d calls=%0d stamp=%0d", y_cont, z_cont, cont_calls > 101, stamp);
    stamp = 7;
    #1 $display("procedural_writer stamp=%0d z=%0d", stamp, z_cont);
    $finish(0);
  end
endmodule
