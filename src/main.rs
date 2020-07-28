use rx::{Observable, Subscriber, Subscription};
use std::rc::Rc;
use std::sync::Arc;

mod rx {
  use std::any::Any;
  use std::cell::Cell;
  use std::rc::Rc;
  use std::sync::Arc;

  pub struct GroupSubscription(Vec<Subscription>);

  impl Default for GroupSubscription {
    fn default() -> Self {
      GroupSubscription(Vec::<Subscription>::new())
    }
  }

  impl GroupSubscription {
    fn add(&mut self, subscription: Subscription) {
      self.0.push(subscription);
    }

    fn unsubscribe(&mut self) {
      for elem in self.0.iter() {
        elem.unsubscribe();
      }
    }
  }

  #[derive(Clone)]
  pub struct Subscription {
    unsubscribe: fn(),
  }
  impl Subscription {
    pub fn new(unsubscribe: fn()) -> Self {
      Subscription { unsubscribe }
    }

    pub fn unsubscribe(&self) {
      (self.unsubscribe)();
    }

    pub fn set_unsubscribe(&mut self, unsubscribe: fn()) {
      self.unsubscribe = unsubscribe;
    }
  }

  #[derive(Clone)]
  pub struct Observer<T> {
    next: Rc<dyn Fn(T)>,
    error: Rc<dyn Fn(Box<dyn Any>)>,
    complete: Rc<dyn Fn()>,
  }
  impl<T> Observer<T> {
    pub fn new(
      next: Rc<dyn Fn(T)>,
      error: Rc<dyn Fn(Box<dyn Any>)>,
      complete: Rc<dyn Fn()>,
    ) -> Self {
      Observer {
        next,
        error,
        complete,
      }
    }

    pub fn next(&self, value: T) {
      (self.next)(value);
    }

    pub fn error(&self, error: Box<dyn Any>) {
      (self.error)(error);
    }

    pub fn complete(&self) {
      (self.complete)();
    }
  }

  #[derive(Clone)]
  pub struct Subscriber<T> {
    subscription: Subscription,
    observer: Observer<T>,
  }
  impl<T> Subscriber<T> {
    pub fn new(subscription: Subscription, observer: Observer<T>) -> Self {
      Subscriber {
        subscription,
        observer,
      }
    }
    pub fn next(&self, value: T) {
      self.observer.next(value);
    }

    pub fn error(&self, error: Box<dyn Any>) {
      self.observer.error(error);
      self.subscription.unsubscribe();
    }

    pub fn complete(&self) {
      self.observer.complete();
      self.subscription.unsubscribe();
    }
  }

  #[derive(Clone)]
  pub struct Observable<T> {
    subscribe: Rc<dyn Fn(Rc<Subscriber<T>>) -> Subscription>,
  }

  impl<T> Observable<T> {
    pub fn new(subscribe: Rc<dyn Fn(Rc<Subscriber<T>>) -> Subscription>) -> Self {
      Observable { subscribe }
    }
    pub fn subscribe(&self, subscriber: Rc<Subscriber<T>>) -> Subscription {
      return (self.subscribe)(subscriber);
    }
    pub fn pipe<B>(self, operator: Box<dyn Fn(Observable<T>) -> Observable<B>>) -> Observable<B> {
      return operator(self);
    }
  }

  pub fn map<A: 'static, B: 'static>(
    transform_fn: fn(A) -> B,
  ) -> Box<dyn Fn(Observable<A>) -> Observable<B>> {
    Box::new(move |in_observable: Observable<A>| {
      Observable::<B>::new(Rc::new(move |out_observer: Rc<Subscriber<B>>| {
        let out_observer_clone_next = out_observer.clone();
        let out_observer_clone_error = out_observer.clone();
        let out_observer_clone_complete = out_observer.clone();
        in_observable.subscribe(Rc::new(Subscriber::new(
          Subscription::new(|| println!("map unsub")),
          Observer {
            next: Rc::new(move |x| out_observer_clone_next.next(transform_fn(x))),
            error: Rc::new(move |e| out_observer_clone_error.error(e)),
            complete: Rc::new(move || out_observer_clone_complete.complete()),
          },
        )))
      }))
    })
  }

  pub fn filter<A: 'static + Copy>(
    condition_fn: fn(A) -> bool,
  ) -> Box<dyn Fn(Observable<A>) -> Observable<A>> {
    Box::new(move |in_observable: Observable<A>| {
      Observable::<A>::new(Rc::new(move |out_observer: Rc<Subscriber<A>>| {
        let out_observer_clone_next = out_observer.clone();
        let out_observer_clone_error = out_observer.clone();
        let out_observer_clone_complete = out_observer.clone();
        in_observable.subscribe(Rc::new(Subscriber::new(
          Subscription::new(|| println!("filter unsub")),
          Observer {
            next: Rc::new(move |x| {
              if condition_fn(x) {
                out_observer_clone_next.next(x);
              }
            }),
            error: Rc::new(move |e| out_observer_clone_error.error(e)),
            complete: Rc::new(move || out_observer_clone_complete.complete()),
          },
        )))
      }))
    })
  }

  pub fn switch_map<A: 'static + Copy, B: 'static>(
    transform_fn: Rc<Fn(A) -> Observable<B>>,
  ) -> Box<dyn Fn(Observable<A>) -> Observable<B>> {
    Box::new(move |in_observable: Observable<A>| {
      let tf_clone = transform_fn.clone();
      Observable::<B>::new(Rc::new(move |out_observer: Rc<Subscriber<B>>| {
        let tf_clone = tf_clone.clone();
        let inner_subscription = Rc::new(Subscription::new(|| {}));
        let this_inner_subscription_clone = inner_subscription.clone();
        let active = Rc::new(Cell::new(1));
        let this_active_clone = Rc::new(Cell::new(1));
        let o1 = out_observer.clone();
        let o2 = out_observer.clone();
        let in_observer = Observer::new(
          Rc::new(move |x| {
            let inner_observable = tf_clone.clone()(x);
            let out_observer_clone_next = out_observer.clone();
            let out_observer_clone_error = out_observer.clone();
            let out_observer_clone_complete = out_observer.clone();
            let inner_observer = Observer::<B>::new(
              Rc::new(move |y| out_observer_clone_next.next(y)),
              Rc::new(move |e| out_observer_clone_error.error(e)),
              Rc::new(move || out_observer_clone_complete.complete()),
            );
            inner_subscription.clone().unsubscribe();

            let _inner_subscription =
              Rc::new(inner_observable.subscribe(Rc::new(Subscriber::new(
                Subscription::new(|| println!("inner subscription")),
                inner_observer,
              ))));

            if let Ok(mut value) = Rc::try_unwrap(inner_subscription.clone()) {
              value.set_unsubscribe(_inner_subscription.unsubscribe);
            }
          }),
          Rc::new(move |e| o1.error(e)),
          Rc::new(move || {
            let active_clone = active.clone();
            active_clone.set(active.get() - 1);
            if active.get() <= 0 {
              o2.complete();
            }
          }),
        );

        let mut group_subscription = GroupSubscription::default();
        if let Ok(is) = Rc::try_unwrap(this_inner_subscription_clone) {
          group_subscription.add(is);
        }
        this_active_clone.set(this_active_clone.get() + 1);
        group_subscription.add(in_observable.subscribe(Rc::new(Subscriber::new(
          Subscription::new(|| println!("in subscription")),
          in_observer,
        ))));
        return Subscription::new(|| {});
      }))
    })
  }
}

fn from_array<T: 'static + Copy>(array: Vec<T>) -> Observable<T> {
  Observable::new(Rc::new(move |observer: Rc<Subscriber<T>>| {
    let observer_clone = observer.clone();
    for elem in array.iter() {
      observer_clone.next(*elem);
    }
    observer_clone.complete();
    return Subscription::new(|| {});
  }))
}

fn main() {
  use rx::{filter, map, switch_map, Observer};

  let ob1 = from_array(vec![1, 2, 3]);

  let ob2 = from_array(vec![4, 5, 6]).pipe(switch_map(Rc::new(|_| {
    from_array(vec![1, 2, 3])
      .pipe(map(|x| x * 100))
      .pipe(filter(|x| x < 300))
  })));

  let sub1 = Rc::new(Subscriber::new(
    Subscription::new(|| println!("unsubscription1")),
    Observer::new(
      Rc::new(|x| println!("{:?}", x)),
      Rc::new(|_e| {}),
      Rc::new(|| println!("done1")),
    ),
  ));

  ob1.subscribe(sub1.clone());
  ob2.subscribe(sub1);
}
