// Allocation failure matrix: string values, methods, formatting and the
// conversions between strings and packed values.
`timescale 1ns / 1ns
module tb;
  string s, t, parts[4];
  string names[$];
  logic [8*12-1:0] packed_text;
  int n;
  real r;

  function automatic string repeated(string unit, int count);
    string result;
    repeat (count) result = {result, unit, "-"};
    return result;
  endfunction

  initial begin
    s = "allocation";
    t = {s, " ", s.toupper(), " ", s.substr(2, 6)};
    $display("%s %0d", t, t.len());
    t = repeated("ab", 40);
    $display("%0d %s", t.len(), t.substr(110, 119));
    parts[0] = $sformatf("%0d:%h:%b", 123456, 16'hbeef, 5'b10x1z);
    parts[1] = $sformatf("%s/%s", s, parts[0]);
    $sformat(t, "%10s|%-10s|%e", "right", "left", 1234.5);
    parts[2] = t;
    parts[3] = {3{parts[1]}};
    foreach (parts[i]) $display("%0d %s", i, parts[i]);
    n = parts[0].atoi();
    s.itoa(n * 7);
    r = s.atoreal() / 4.0;
    t.realtoa(r);
    $display("%0d %s %s", n, s, t);
    t.hextoa(48879);
    s.bintoa(37);
    $display("%s %s %0d %0d", t, s, t.atohex(), s.atobin());
    packed_text = "packed text";
    s = string'(packed_text);
    packed_text = {packed_text[47:0], "again "};
    $display("%s %h", s, packed_text);
    for (int i = 0; i < 6; i++) names.push_back($sformatf("name_%0d_%s", i, s.substr(0, i)));
    names.sort();
    $display("%s %s %0d", names[0], names[5], names[3].compare(names[2]));
    s = names[4];
    s.putc(0, "N");
    $display("%s %c %0d %0d", s, s.getc(5), s.icompare("NAME_4_PACK"), s == names[4]);
    $display("done");
    $finish;
  end
endmodule
