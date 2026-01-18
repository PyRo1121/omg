import { Component } from 'solid-js';
import { Router, Route } from '@solidjs/router';
import HomePage from './pages/HomePage';
import DashboardPage from './pages/DashboardPage';

const App: Component = () => {
  return (
    <Router>
      <Route path="/" component={HomePage} />
      <Route path="/dashboard" component={DashboardPage} />
    </Router>
  );
};

export default App;
