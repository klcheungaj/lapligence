// 20.2: a $stop executed by a function under a static initializer is held
// until the scheduler starts, before any process runs; the remaining
// initializers complete first.
class reporter;
  static int count;
  int id;
  static function reporter make(bit stop_now);
    count++;
    $display("make %0d", count);
    if (stop_now) $stop;
    make = new;
    make.id = count;
  endfunction
endclass
class user;
  static reporter first = reporter::make(0);
  static reporter second = reporter::make(first.id < 5);
  static reporter third = reporter::make(0);
endclass
module tb;
  initial $display("run %0d %0d %0d %0d", user::first.id, user::second.id,
                   user::third.id, reporter::count);
endmodule
