// AB-O4: a string literal in an untyped mailbox, retrieved into a string.
//
// IEEE 1800-2009 15.4.5 (SystemVerilog-1800-2009.txt L20581-20582): "If the
// type of the message variable is not equivalent to the type of the message
// in the mailbox, the method returns a negative integer." 5.9 (L4174-4176):
// "String literals are packed arrays (of a width that is a multiple of 8
// bits), and they are implicitly converted to the string type when assigned
// to a string type or used in an expression involving string type operands".
//
// Decision (industry-standard behaviour; a literal has no string type until
// its context converts it, and put() names no type): a message that is a
// string literal matches a string variable, which get, peek, try_get and
// try_peek convert it to (6.16: zero bytes are dropped). So does a
// concatenation of literals (Table 6-9) and a parameter without a declared
// type whose value is a literal. A packed variable, an expression with another
// operand, a conditional expression and a typed parameter stay packed: they do
// not match a string.
module tb;
  mailbox m = new();
  string t;
  class holder;
    string text;
  endclass
  holder h = new;
  int r;
  bit [39:0] packed_text = "seven";
  localparam UNTYPED = "untyped";
  localparam JOINED = {"jo", "ined"};
  localparam MIXED = {"ab", 8'h41};
  localparam bit [39:0] TYPED = "typed";

  task into_string(string name);
    r = m.try_get(t);
    if (r > 0) $display("%s 1 <%s>", name, t);
    else begin
      $display("%s %0d", name, r);
      m = new();
    end
  endtask

  initial begin
    fork
      begin
        m.get(t);
        $display("blocked get <%s> %0d", t, t.len());
      end
    join_none
    #1 m.put("late");
    #1 m.put("seven");
    m.peek(t);
    $display("peek <%s>", t);
    r = m.try_peek(t);
    $display("try_peek %0d <%s>", r, t);
    m.get(t);
    $display("get <%s> %0d", t, m.num());
    fork
      begin
        m.peek(t);
        $display("blocked peek <%s> %0d", t, m.num());
        m.get(h.text);
        $display("property get <%s> %0d", h.text, m.num());
        m.get(h.text);
        $display("blocked property get <%s>", h.text);
      end
    join_none
    #1 m.put("held");
    #1 m.put("last");
    #1;
    m.put("first");
    m.put("second");
    m.peek(t);
    r = m.try_peek(t);
    $display("queued peek <%s> %0d", t, m.num());
    into_string("queued first");
    into_string("queued second");
    m.put(("paren"));
    into_string("parentheses");
    m.put({"con", "cat"});
    into_string("concatenation");
    m.put({2{"ab"}});
    into_string("replication");
    m.put("");
    into_string("empty");
    m.put(UNTYPED);
    into_string("untyped parameter");
    m.put(JOINED);
    into_string("concatenated parameter");
    r = m.try_put("tried");
    into_string("try_put");
    m.put(packed_text);
    into_string("packed variable");
    m.put({"ab", 8'h41});
    into_string("mixed concatenation");
    m.put(MIXED);
    into_string("mixed parameter");
    m.put(TYPED);
    into_string("typed parameter");
    m.put(r ? "yes" : "no");
    into_string("conditional");
    m.put(7);
    into_string("integer");
  end
endmodule
