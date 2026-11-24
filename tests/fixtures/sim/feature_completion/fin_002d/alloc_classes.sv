// Allocation failure matrix: class objects with native members, handle
// graphs with cycles, and the collector reclaiming them while the model
// keeps allocating.
`timescale 1ns / 1ns
package shapes;
  virtual class shape;
    string name;
    int tags[$];
    shape next;
    function new(string label);
      name = label;
    endfunction
    pure virtual function int area();
    virtual function string describe();
      return $sformatf("%s:%0d", name, area());
    endfunction
  endclass

  class square extends shape;
    int side;
    logic [149:0] mask;
    function new(int length);
      super.new($sformatf("square%0d", length));
      side = length;
      mask = 150'd1 << length;
    endfunction
    virtual function int area();
      return side * side + int'(mask[side]) - 1;
    endfunction
  endclass

  class strip extends shape;
    int cells[];
    int lookup[string];
    function new(int length);
      super.new($sformatf("strip%0d", length));
      cells = new[length];
      foreach (cells[i]) cells[i] = i + 1;
      lookup[name] = length;
    endfunction
    virtual function int area();
      return cells.sum();
    endfunction
    virtual function string describe();
      return {super.describe(), "!"};
    endfunction
  endclass

  class registry #(type T = shape);
    static int created;
    T entries[$];
    T by_name[string];
    function void add(T entry);
      entries.push_back(entry);
      by_name[entry.name] = entry;
      created++;
    endfunction
  endclass
endpackage

module tb;
  import shapes::*;
  registry #(shape) all;
  shape ring, current;
  square sq;
  strip st;
  int total;

  initial begin
    all = new;
    for (int i = 1; i <= 6; i++) begin
      sq = new(i);
      st = new(i + 2);
      sq.tags.push_back(i);
      all.add(sq);
      all.add(st);
    end
    foreach (all.entries[i]) total += all.entries[i].area();
    $display("%0d %0d %s %s", registry#(shape)::created, total, all.entries[3].describe(),
             all.by_name["square5"].describe());
    // Rings of objects that only refer to each other: garbage once dropped.
    for (int round = 0; round < 40; round++) begin
      ring = null;
      for (int i = 0; i < 12; i++) begin
        if (i % 2 == 0) begin
          sq = new(i);
          current = sq;
        end else begin
          st = new(i);
          current = st;
        end
        current.next = ring;
        ring = current;
      end
      for (current = ring; current.next != null; current = current.next) total += current.area();
      current.next = ring;
    end
    $display("%0d %s", total, ring.next.next.describe());
    if ($cast(st, all.entries[1])) $display("cast %0d %0d", st.cells.size(), st.lookup["strip3"]);
    if (!$cast(sq, all.entries[1])) $display("no cast");
    all = null;
    ring = null;
    #1;
    $display("done");
    $finish;
  end
endmodule
