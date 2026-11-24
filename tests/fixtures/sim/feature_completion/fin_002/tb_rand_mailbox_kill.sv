// FIN-002 testbench scenario 4: randomized packets (inheritance, virtual
// dispatch, `randc`, distributions, implications, `pre_randomize` and
// `post_randomize`) travel through a bounded mailbox inside a parameterized
// channel class with static members. The first consumer is killed by
// `disable fork` while it holds a packet and a semaphore key; a second
// consumer takes over. Processes killed while blocked in a mailbox `get`
// and a semaphore `get` must not swallow later messages or keys. Random
// values differ between tools, so only invariants are printed; every
// expected line is derived in readme.md.
`timescale 1ns / 1ns

package pkt_pkg;
  typedef enum bit [1:0] {
    DATA,
    CTRL
  } kind_t;

  virtual class item;
    static int created;
    int serial;
    function new();
      serial = created++;
    endfunction
    pure virtual function string describe();
    virtual function int cost();
      return 1;
    endfunction
  endclass

  class packet extends item;
    rand bit [7:0] len;
    rand bit [3:0] dst;
    rand kind_t kind;
    randc bit [2:0] tag;
    rand bit [7:0] payload[];
    int pre_calls;
    int post_calls;
    bit [7:0] checksum;
    constraint c_len {
      len inside {[1 : 8]};
      payload.size() == len;
    }
    constraint c_dst {
      dst dist {
        0 := 1,
        [1 : 3] :/ 3,
        15 := 0
      };
    }
    constraint c_kind {
      kind dist {
        DATA := 8,
        CTRL := 2
      };
      (kind == CTRL) -> (len <= 2);
      solve kind before len;
    }
    function void pre_randomize();
      pre_calls++;
    endfunction
    function void post_randomize();
      post_calls++;
      checksum = '0;
      foreach (payload[i]) checksum ^= payload[i];
    endfunction
    virtual function string describe();
      return $sformatf("packet#%0d", serial);
    endfunction
    virtual function int cost();
      return int'(len);
    endfunction
    // The declared constraints, checked procedurally.
    virtual function bit legal();
      if (!(len inside {[1 : 8]})) return 1'b0;
      if (payload.size() != int'(len)) return 1'b0;
      if (!(dst inside {[0 : 3]})) return 1'b0;
      if (kind == CTRL && len > 2) return 1'b0;
      return 1'b1;
    endfunction
  endclass

  class big_packet extends packet;
    constraint c_big {len >= 6;}
    virtual function string describe();
      return $sformatf("big#%0d", serial);
    endfunction
    virtual function bit legal();
      return super.legal() && len >= 6 && kind == DATA;
    endfunction
  endclass

  class seeded;
    rand bit [15:0] word;
  endclass

  class channel #(
      type T = int,
      int DEPTH = 2
  );
    static int instances;
    mailbox #(T) box;
    int puts;
    int gets;
    function new();
      box = new(DEPTH);
      instances++;
    endfunction
    task put(T value);
      box.put(value);
      puts++;
    endtask
    task get(output T value);
      box.get(value);
      gets++;
    endtask
  endclass
endpackage

module tb;
  import pkt_pkg::*;

  channel #(packet) link = new();
  channel #(int, 4) numbers = new();
  channel #(int, 4) spare = new();
  semaphore key = new(1);
  packet taken_a[$];
  packet taken_b[$];
  int illegal;
  int by_kind[kind_t];
  process consumer_a;

  task automatic generate_packets(int count);
    packet p;
    big_packet b;
    for (int i = 0; i < count; i++) begin
      if (i % 3 == 2) begin
        b = new();
        p = b;
      end else p = new();
      if (!p.randomize()) illegal++;
      if (!p.legal()) illegal++;
      link.put(p);
      #2;
    end
  endtask

  task automatic consume(input bit locked, ref packet taken[$]);
    packet p;
    forever begin
      link.get(p);
      taken.push_back(p);
      if (locked) key.get(1);
      #5;
      by_kind[p.kind]++;
      if (locked) key.put(1);
    end
  endtask

  function automatic string names(packet list[$]);
    string text;
    foreach (list[i]) text = {text, i == 0 ? "" : " ", list[i].describe()};
    return text;
  endfunction

  initial begin
    packet probe;
    big_packet big;
    seeded plain;
    process self;
    bit [7:0] seen;
    bit randc_ok;
    int first;
    int value;
    int cost;
    by_kind[DATA] = 0;
    by_kind[CTRL] = 0;
    fork
      generate_packets(24);
    join_none
    // Consumer A is killed at 23 by `disable fork` of its parent alone.
    fork
      begin
        fork
          begin
            consumer_a = process::self();
            consume(1'b1, taken_a);
          end
        join_none
        #23 disable fork;
      end
    join
    $display("[%0t] A killed: status=%s took=%0d", $time, consumer_a.status().name(), taken_a.size(
             ));
    $display("  A took %s", names(taken_a));
    $display("  key free=%0d queued=%0d", key.try_get(1), link.box.num());
    #2;
    fork
      consume(1'b0, taken_b);
    join_none
    wait (taken_b.size() == 19);
    #5;
    $display("[%0t] B took=%0d first=%s last=%s", $time, taken_b.size(), taken_b[0].describe(),
             taken_b[$].describe());
    foreach (taken_a[i]) cost += taken_a[i].cost();
    foreach (taken_b[i]) cost += taken_b[i].cost();
    $display("  created=%0d puts=%0d gets=%0d illegal=%0d cost in range=%0d", item::created,
             link.puts, link.gets, illegal, cost >= 24 && cost <= 24 * 8);
    $display("  kinds counted=%0d", by_kind[DATA] + by_kind[CTRL]);

    // A process killed while blocked in get() takes nothing afterwards.
    fork
      begin
        fork
          begin
            numbers.get(value);
            $display("unreachable: the killed reader got %0d", value);
          end
          begin
            key.get(1);
            $display("unreachable: the killed process got the key");
          end
        join_none
        #1 disable fork;
      end
    join
    numbers.put(7);
    key.put(1);
    #1;
    $display("[%0t] after killed waiters: queued=%0d key=%0d value=%0d", $time, numbers.box.num(),
             key.try_get(1), value);
    $display("  channel instances: packet=%0d int=%0d", channel#(packet)::instances,
             channel#(int, 4)::instances);

    // randc: every cycle of eight calls is a permutation of the values.
    probe = new();
    randc_ok = 1'b1;
    repeat (2) begin
      seen = '0;
      repeat (8) begin
        if (!probe.randomize()) illegal++;
        if (seen[probe.tag]) randc_ok = 1'b0;
        seen[probe.tag] = 1'b1;
      end
    end
    $display("randc permutations=%0d pre=%0d post=%0d", randc_ok, probe.pre_calls,
             probe.post_calls);

    // Inline constraints, switched-off variables and an unsatisfiable set.
    if (!probe.randomize() with {
          len == 3;
          kind == DATA;
        })
      illegal++;
    $display("inline len=%0d size=%0d kind=%s", probe.len, probe.payload.size(), probe.kind.name());
    probe.len.rand_mode(0);
    probe.c_len.constraint_mode(0);
    if (!probe.randomize()) illegal++;
    $display("frozen len=%0d", probe.len);
    big = new();
    first = big.randomize() with {kind == CTRL;};
    $display("conflict returns %0d pre=%0d post=%0d", first, big.pre_calls, big.post_calls);

    // Object and thread stability: the same seed replays the same values.
    plain = new();
    plain.srandom(5);
    void'(plain.randomize());
    first = int'(plain.word);
    plain.srandom(5);
    void'(plain.randomize());
    self = process::self();
    self.srandom(9);
    value = $urandom();
    self.srandom(9);
    $display("replay object=%0d thread=%0d", first == int'(plain.word), value == $urandom());
    void'(std::randomize(value) with {value inside {[40 : 42]};});
    $display("scope value in range=%0d illegal=%0d", value inside {[40 : 42]}, illegal);
    $finish;
  end
endmodule
