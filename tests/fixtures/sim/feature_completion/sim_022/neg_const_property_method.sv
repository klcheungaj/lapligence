// SIM-022-A03 nearest illegal: delete writes a const associative-array
// class property reached through an object.
module tb;
  class Table;
    const int entries[int] = '{1: 10};
  endclass
  initial begin
    Table t = new;
    t.entries.delete(1);
  end
endmodule
