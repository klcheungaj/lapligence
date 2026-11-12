// Inline constraints that name the randomized object through the
// receiver's own path (`req.randomize() with { req.addr == a; }`), as a
// sequence's `uvm_do_with` does: the names are the object's random
// variables. A path naming another object reads that object's values.
class item;
  rand int unsigned size;
  rand bit [15:0] addr;
  rand bit [7:0] data[];
  rand bit [3:0] kind;
  constraint c_size { size inside {1, 2, 4, 8}; }
  constraint c_data { data.size() == size; }
endclass

class holder;
  item req;
endclass

class seq;
  item req;
  item other;
  holder h;
  bit [15:0] start_addr = 16'h1234;

  function void body();
    req = new;
    other = new;
    other.kind = 4'd9;
    other.size = 3;
    if (!req.randomize() with { req.addr == start_addr; req.size == 4;
                                req.data.size() == 4; req.kind == other.kind; })
      $display("randomize 1 failed");
    $display("1: size=%0d addr=%h n=%0d kind=%0d", req.size, req.addr, req.data.size(), req.kind);
    if (!this.req.randomize() with { req.size == 2; addr == 16'h00ff; })
      $display("randomize 2 failed");
    $display("2: size=%0d addr=%h n=%0d", req.size, req.addr, req.data.size());
    h = new;
    h.req = req;
    if (!h.req.randomize() with { h.req.size == 8; h.req.addr == other.size; })
      $display("randomize 3 failed");
    $display("3: size=%0d addr=%h n=%0d", req.size, req.addr, req.data.size());
    // `other` is not the randomized object: its size is a state value.
    if (!req.randomize() with { size == other.size + 1; })
      $display("randomize 4 failed");
    $display("4: size=%0d other.size=%0d", req.size, other.size);
  endfunction
endclass

module tb;
  initial begin
    seq s = new;
    s.body();
  end
endmodule
