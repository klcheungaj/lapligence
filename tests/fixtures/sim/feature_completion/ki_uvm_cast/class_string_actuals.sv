class ada;
  virtual function string rights(int i);
    return i == 0 ? "RW" : i == 1 ? "RO" : "NA";
  endfunction
endclass
class base;
  string msg = "m";
  protected string dflt;
  string table_[2];
  function bit get_cfg(string n, inout string v);
    v = {n, "!", v};
    return 1;
  endfunction
  function void set_out(output string v, input string n);
    v = n;
  endfunction
endclass
class seqb extends base;
  function void pm();
    $swrite(msg, "%s%s", msg, "x");
    $display("%s", msg);
  endfunction
  function void cfg();
    dflt = "o";
    if (get_cfg("d", dflt)) $display("%s", dflt);
    set_out(dflt, "out");
    $display("%s", dflt);
    if (get_cfg("t", this.msg)) $display("%s", msg);
  endfunction
endclass
module tb;
  ada ad = new;
  seqb s = new;
  function automatic void pick(ada a, int i);
    case (a.rights(i))
      "RW": $display("rw");
      "RO", "WO": $display("ro");
      default: $display("other %s", a.rights(i));
    endcase
  endfunction
  initial begin
    pick(ad, 0);
    pick(ad, 1);
    pick(ad, 2);
    s.pm();
    s.cfg();
    s.set_out(s.msg, "external");
    $display("%s", s.msg);
    $finish;
  end
endmodule
