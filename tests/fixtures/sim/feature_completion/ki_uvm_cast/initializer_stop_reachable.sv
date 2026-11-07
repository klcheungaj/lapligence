// 20.2: static initializers call a function that holds $stop on a path the
// calls do not take; the initializers run in the static schedule.
class reporter;
  static int count;
  int id;
  static function reporter make(bit stop_now);
    count++;
    if (stop_now) begin
      $display("stopping");
      $stop;
    end
    make = new;
    make.id = count;
  endfunction
endclass
class user;
  static reporter first = reporter::make(0);
  static reporter second = reporter::make(first.id > 5);
endclass
module tb;
  initial $display("%0d %0d %0d", user::first.id, user::second.id, reporter::count);
endmodule
